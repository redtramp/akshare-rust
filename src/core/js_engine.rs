//! JS 引擎封装（rquickjs / QuickJS）。
//!
//! 对应 akshare 的 `py_mini_racer`（V8）：在进程内执行网站下发的加密 JS，
//! 例如巨潮 `cninfo.js::getResCode1()`、同花顺 `ths.js::v()`。
//!
//! 已知差异：rquickjs 的 eval 默认严格模式，而 akshare 的 JS 依赖
//! 隐式全局（`localStorage=null`、`BROWSER_LIST={}` 等），
//! 因此按文件注入 shim 前缀（实测与 py_mini_racer 输出逐字符一致）。

use crate::core::error::{AkshareError, Result};
use rquickjs::{CatchResultExt, Context, Runtime, Value};

/// 各 JS 文件所需的浏览器全局 shim（严格模式 vs V8 sloppy 模式的差异）。
const SHIMS: &[(&str, &str)] = &[
    ("cninfo.js", "var localStorage = null;\n"),
    (
        "ths.js",
        "var BROWSER_LIST = {}; var time; var plugin_num;\n",
    ),
    ("outcrypto.js", ""),
    ("jm.js", ""),
    ("crypto.js", ""),
];

/// 简易 JS 执行环境（每个实例一个独立 QuickJS 上下文）。
pub struct JsEngine {
    _rt: Runtime,
    ctx: Context,
}

impl JsEngine {
    /// 新建独立 JS 运行时。
    pub fn new() -> Result<Self> {
        let rt = Runtime::new().map_err(|e| AkshareError::js(e.to_string()))?;
        let ctx = Context::full(&rt).map_err(|e| AkshareError::js(e.to_string()))?;
        Ok(Self { _rt: rt, ctx })
    }

    /// 加载 JS 文件（自动注入该文件所需 shim），然后执行 `expr` 并返回字符串结果。
    ///
    /// `expr` 在 JS 内部用 try/catch 包装，异常以 `JSERR: <message>` 前缀返回，
    /// 便于提取真实错误信息。
    pub fn load_and_call(&mut self, js_name: &str, js_code: &str, expr: &str) -> Result<String> {
        // 1) 注入 shim + 加载 JS
        let prelude = SHIMS
            .iter()
            .find(|(name, _)| *name == js_name)
            .map(|(_, shim)| *shim)
            .unwrap_or("");
        let wrapped = format!("{prelude}\n{js_code}");

        // 闭包内将结果/错误转为 owned String，避免 CaughtError 生命周期逃逸
        let load_result: std::result::Result<(), String> =
            self.ctx
                .with(|ctx| match ctx.eval::<Value, _>(wrapped).catch(&ctx) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!("加载 {js_name} 失败: {e}")),
                });
        load_result.map_err(AkshareError::js)?;

        // 2) 执行表达式
        let script = format!(
            "(function(){{ try {{ return String({expr}); }} catch(e) {{ return 'JSERR: ' + e.message; }} }})()"
        );
        let out: std::result::Result<String, String> =
            self.ctx
                .with(|ctx| match ctx.eval::<String, _>(script).catch(&ctx) {
                    Ok(v) => Ok(v),
                    Err(e) => Err(e.to_string()),
                });
        match out {
            Ok(v) if v.starts_with("JSERR: ") => Err(AkshareError::js(
                v.trim_start_matches("JSERR: ").to_string(),
            )),
            Ok(v) => Ok(v),
            Err(e) => Err(AkshareError::js(format!("执行 {expr} 失败: {e}"))),
        }
    }

    /// 直接执行一段 JS 并返回字符串结果（无 shim，用于调试/单测）。
    pub fn call_expr(&mut self, expr: &str) -> Result<String> {
        let script = format!(
            "(function(){{ try {{ return String({expr}); }} catch(e) {{ return 'JSERR: ' + e.message; }} }})()"
        );
        let out: std::result::Result<String, String> =
            self.ctx
                .with(|ctx| match ctx.eval::<String, _>(script).catch(&ctx) {
                    Ok(v) => Ok(v),
                    Err(e) => Err(e.to_string()),
                });
        match out {
            Ok(v) if v.starts_with("JSERR: ") => Err(AkshareError::js(
                v.trim_start_matches("JSERR: ").to_string(),
            )),
            Ok(v) => Ok(v),
            Err(e) => Err(AkshareError::js(format!("执行 {expr} 失败: {e}"))),
        }
    }
}

/// 巨潮加密头（对应 akshare `getResCode1()`）。
///
/// AES-CBC(时间戳, key="1234567887654321") → base64。
/// 每次调用返回不同的 24 字符 token（基于时间戳）。
pub fn cninfo_get_res_code() -> Result<String> {
    let js = include_str!("../../assets/js/cninfo.js");
    let mut engine = JsEngine::new()?;
    engine.load_and_call("cninfo.js", js, "getResCode1()")
}

/// 同花顺 token（对应 akshare `v()`）。
pub fn ths_get_v() -> Result<String> {
    let js = include_str!("../../assets/js/ths.js");
    let mut engine = JsEngine::new()?;
    engine.load_and_call("ths.js", js, "v()")
}

/// 预置 JS 资源清单（供 assets 校验/未来扩展）。
pub fn available_js_files() -> Vec<&'static str> {
    SHIMS.iter().map(|(name, _)| *name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn cninfo_get_res_code_format() {
        // 离线可用：cninfo.js 已打包进 crate
        let v = cninfo_get_res_code().expect("cninfo.js 应可执行");
        // base64 长度 24（16 字节 AES 块）
        assert_eq!(v.len(), 24, "token 应为 24 字符 base64");
    }

    #[test]
    fn ths_get_v_format() {
        let v = ths_get_v().expect("ths.js 应可执行");
        // 实测与 py_mini_racer 输出一致：60 字符 token
        assert_eq!(v.len(), 60, "token 应为 60 字符");
    }

    #[test]
    fn engine_isolated_contexts() {
        let mut e1 = JsEngine::new().unwrap();
        let mut e2 = JsEngine::new().unwrap();
        assert_eq!(e1.call_expr("1+1").unwrap(), "2");
        assert_eq!(e2.call_expr("2*3").unwrap(), "6");
    }

    #[test]
    fn js_error_is_readable() {
        let mut e = JsEngine::new().unwrap();
        let r = e.call_expr("undefinedVar.bar");
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("undefinedVar"));
    }

    #[test]
    fn shim_registry_has_all_known_files() {
        let files = available_js_files();
        assert!(files.contains(&"cninfo.js"));
        assert!(files.contains(&"ths.js"));
        assert_eq!(files.len(), 5);
    }

    #[test]
    fn shims_map_is_consistent() {
        // 确保所有文件都有对应条目
        let names: HashMap<_, _> = SHIMS.iter().copied().collect();
        assert_eq!(names.len(), SHIMS.len());
        for (name, _) in SHIMS {
            assert!(names.contains_key(name));
        }
    }
}
