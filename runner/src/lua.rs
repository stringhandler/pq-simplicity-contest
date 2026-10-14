//! Running an entry's `convert.lua` in a sandbox.
//!
//! Each call gets a fresh Lua state with only `string`, `table`, `math` and `utf8`. Base
//! functions that reach outside, or load code (`load`, `loadfile`, `dofile`, `require`,
//! `collectgarbage`, `print`), are removed. There is a memory limit and an instruction
//! budget. A fresh state per call also means a converter can't carry anything from one
//! signature to the next.
//!
//! The instruction budget only counts Lua instructions: a C function runs to the end
//! before the hook sees it. So the string pattern functions, which can backtrack for hours
//! on one call, are removed too. The script must be source text: Lua doesn't check
//! bytecode, and bad bytecode can corrupt memory.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use mlua::chunk::ChunkMode;
use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};

use crate::signer::RawSig;

const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
const INSTRUCTION_LIMIT: u64 = 50_000_000;
const HOOK_EVERY: u32 = 10_000;

/// Call `convert(sig)` in `script`, returning witness name → SimplicityHL value.
pub fn convert(script: &str, sig: &RawSig) -> Result<BTreeMap<String, String>> {
    let lua = Lua::new_with(
        StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8,
        LuaOptions::default(),
    )
    .map_err(|e| anyhow!("cannot start Lua: {e}"))?;
    lua.set_memory_limit(MEMORY_LIMIT)
        .map_err(|e| anyhow!("cannot limit Lua's memory: {e}"))?;

    let used = Arc::new(AtomicU64::new(0));
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(HOOK_EVERY),
        move |_, _| {
            if used.fetch_add(HOOK_EVERY as u64, Ordering::Relaxed) > INSTRUCTION_LIMIT {
                Err(mlua::Error::runtime(
                    "convert.lua exceeded its instruction limit",
                ))
            } else {
                Ok(VmState::Continue)
            }
        },
    )
    .map_err(|e| anyhow!("cannot limit Lua's instructions: {e}"))?;

    let globals = lua.globals();
    for name in [
        "load",
        "loadfile",
        "dofile",
        "require",
        "collectgarbage",
        "print",
    ] {
        globals
            .set(name, Value::Nil)
            .map_err(|e| anyhow!("cannot sandbox Lua: {e}"))?;
    }
    // Strings' methods (`s:find(...)`) are this same table, so this covers them too.
    let string: Table = globals
        .get("string")
        .map_err(|e| anyhow!("cannot sandbox Lua: {e}"))?;
    for name in ["find", "match", "gmatch", "gsub"] {
        string
            .set(name, Value::Nil)
            .map_err(|e| anyhow!("cannot sandbox Lua: {e}"))?;
    }

    lua.load(script)
        .set_name("convert.lua")
        .set_mode(ChunkMode::Text)
        .exec()
        .map_err(|e| anyhow!("convert.lua: {e}"))?;
    let convert: Function = globals
        .get("convert")
        .map_err(|_| anyhow!("convert.lua defines no function `convert`"))?;

    let t = lua.create_table().map_err(|e| anyhow!("{e}"))?;
    let set = |k: &str, v: Value| t.set(k, v).map_err(|e| anyhow!("{e}"));
    let s = |v: &str| -> Result<Value> {
        Ok(Value::String(
            lua.create_string(v).map_err(|e| anyhow!("{e}"))?,
        ))
    };
    let list = |items: &[String]| -> Result<Value> {
        Ok(Value::Table(
            lua.create_sequence_from(items.iter().map(String::as_str))
                .map_err(|e| anyhow!("{e}"))?,
        ))
    };
    set("kind", s("stateful")?)?;
    set("q", Value::Integer(sig.q as i64))?;
    set("msg", s(&sig.msg)?)?;
    set("pk", s(&sig.pk)?)?;
    set("pk_seed", s(&sig.pk_seed)?)?;
    set("pk_root", s(&sig.pk_root)?)?;
    set("raw", s(&sig.raw)?)?;
    set("sl_root", s(&sig.sl_root)?)?;
    set("r", s(&sig.r)?)?;
    set("counter", s(&sig.counter)?)?;
    set("chains", list(&sig.chains)?)?;
    set("path", list(&sig.path)?)?;

    let out: Table = convert.call(t).map_err(|e| anyhow!("convert.lua: {e}"))?;
    let mut witnesses = BTreeMap::new();
    for pair in out.pairs::<String, String>() {
        let (name, value) = pair.map_err(|e| {
            anyhow!("convert.lua must return a table of witness name to value string: {e}")
        })?;
        witnesses.insert(name, value);
    }
    Ok(witnesses)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig() -> RawSig {
        RawSig {
            q: 1,
            msg: "00".repeat(32),
            pk: "11".repeat(32),
            pk_seed: "11".repeat(16),
            pk_root: "11".repeat(16),
            raw: String::new(),
            sl_root: "22".repeat(16),
            r: "33".repeat(32),
            counter: "44".repeat(4),
            chains: vec!["55".repeat(16); 64],
            path: vec!["66".repeat(16)],
        }
    }

    #[test]
    fn converts() {
        let script = r#"function convert(sig) return { ROOT = "0x" .. sig.sl_root } end"#;
        let out = convert(script, &sig()).unwrap();
        assert_eq!(out["ROOT"], format!("0x{}", "22".repeat(16)));
    }

    #[test]
    fn has_no_pattern_functions() {
        for call in [
            r#"string.find("ab", "b")"#,
            r#"("ab"):find("b")"#,
            r#"("ab"):match("b")"#,
            r#"("ab"):gsub("b", "c")"#,
            r#"("ab"):gmatch("b")"#,
        ] {
            let script = format!("function convert(sig) {call} return {{}} end");
            assert!(convert(&script, &sig()).is_err(), "{call} should fail");
        }
    }

    #[test]
    fn refuses_bytecode() {
        let lua = Lua::new();
        let bytecode = lua
            .load("function convert(sig) return {} end")
            .into_function()
            .unwrap()
            .dump(true);
        // Lua's header isn't valid UTF-8, so only a lossy copy reaches here. Its first byte
        // still marks it as bytecode, which text mode refuses before reading the header.
        let script = String::from_utf8_lossy(&bytecode);
        let err = convert(&script, &sig()).unwrap_err().to_string();
        assert!(err.contains("binary chunk"), "{err}");
    }
}
