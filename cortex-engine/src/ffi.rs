/// C-compatible and JNI FFI surface for the Cortex engine.
///
/// C API (used by any embedding host):
///   char* cortex_compile(const char* source, const char* allow_csv);
///   void  cortex_free_string(char* ptr);
///
/// Android JNI (called from CortexEngine.kt via System.loadLibrary):
///   Java_nz_co_icb_cortex_android_engine_CortexEngine_nativeCompile
///
/// The compile call returns a JSON AST string on success, or a JSON error
/// object `{"error": "..."}` on failure. The Go daemon's AST-walking
/// interpreter already knows how to evaluate this JSON, so the Android
/// Kotlin interpreter uses the same format.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::checker::PermissionManifest;

// ── C API ────────────────────────────────────────────────────────────────────

/// Compile `source` to a JSON AST string.
///
/// `allow_csv` is a comma-separated list of permitted native APIs
/// (e.g. `"native.log,native.db.append"`). Pass an empty string to
/// allow all APIs.
///
/// Returns a heap-allocated C string. The caller must free it with
/// `cortex_free_string`. Returns a JSON error object on failure.
#[no_mangle]
pub extern "C" fn cortex_compile(source: *const c_char, allow_csv: *const c_char) -> *mut c_char {
    let source = unsafe {
        if source.is_null() {
            return error_cstring("source pointer is null");
        }
        CStr::from_ptr(source).to_string_lossy().into_owned()
    };

    let allow: Vec<String> = unsafe {
        if allow_csv.is_null() {
            vec![]
        } else {
            CStr::from_ptr(allow_csv)
                .to_string_lossy()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        }
    };

    compile_to_json(&source, allow)
}

/// Free a C string previously returned by `cortex_compile`.
#[no_mangle]
pub extern "C" fn cortex_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

// ── Android JNI ──────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android {
    use super::compile_to_json;
    use jni::objects::{JClass, JString};
    use jni::sys::jstring;
    use jni::JNIEnv;

    /// JNI entry point for `CortexEngine.nativeCompile(source, allow)`.
    #[no_mangle]
    pub extern "system" fn Java_nz_co_icb_cortex_android_engine_CortexEngine_nativeCompile<
        'local,
    >(
        mut env: JNIEnv<'local>,
        _class: JClass<'local>,
        source: JString<'local>,
        allow: JString<'local>,
    ) -> jstring {
        let source: String = match env.get_string(&source) {
            Ok(s) => s.into(),
            Err(e) => return jni_error_string(&mut env, &e.to_string()),
        };
        let allow_str: String = match env.get_string(&allow) {
            Ok(s) => s.into(),
            Err(e) => return jni_error_string(&mut env, &e.to_string()),
        };
        let allow: Vec<String> = allow_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let result_ptr = compile_to_json(&source, allow);

        // Safety: compile_to_json always returns a valid CString.
        let c_str = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
        let java_str = env
            .new_string(c_str.to_string_lossy())
            .expect("failed to create Java string");

        // Free the intermediate C string before returning.
        unsafe {
            drop(std::ffi::CString::from_raw(result_ptr));
        }

        java_str.into_raw()
    }

    fn jni_error_string(env: &mut JNIEnv, msg: &str) -> jstring {
        let json = format!("{{\"error\":{}}}", serde_json::json!(msg));
        env.new_string(json)
            .expect("failed to create error Java string")
            .into_raw()
    }
}

// ── Shared helper ────────────────────────────────────────────────────────────

/// Compile source with the given allow list and return a heap-allocated
/// C string containing the JSON AST or a `{"error":"..."}` object.
fn compile_to_json(source: &str, allow: Vec<String>) -> *mut c_char {
    let manifest = PermissionManifest::new(allow);
    let result = match crate::compile(source, &manifest) {
        Ok(ast) => match serde_json::to_string(&ast) {
            Ok(json) => json,
            Err(e) => format!("{{\"error\":{}}}", serde_json::json!(e.to_string())),
        },
        Err(errs) => {
            let joined = errs.join("\n");
            format!("{{\"error\":{}}}", serde_json::json!(joined))
        }
    };
    CString::new(result)
        .unwrap_or_else(|_| CString::new("{\"error\":\"nul byte in output\"}").unwrap())
        .into_raw()
}

fn error_cstring(msg: &str) -> *mut c_char {
    let json = format!("{{\"error\":{}}}", serde_json::json!(msg));
    CString::new(json)
        .unwrap_or_else(|_| CString::new("{\"error\":\"encoding error\"}").unwrap())
        .into_raw()
}
