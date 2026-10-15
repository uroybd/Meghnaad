//! The functions Kotlin calls (`me.utsob.meghnaad.NativeEngine`). Every engine is a handle, and every call answers
//! in JSON, so a failure is an `error` in the answer and never an exception thrown across the boundary.

use crate::engine::Engine;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jbyteArray, jint, jlong, jstring};
use jni::JNIEnv;
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

type Handle = Mutex<Engine>;

fn text(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(Into::into).unwrap_or_default()
}

fn answer(env: &JNIEnv, value: impl ToString) -> jstring {
    env.new_string(value.to_string())
        .map_or(std::ptr::null_mut(), |s| s.into_raw())
}

/// The engine behind a handle, locked for the length of one call.
fn with_engine<T>(handle: jlong, f: impl FnOnce(&mut Engine) -> T) -> Result<T, String> {
    if handle == 0 {
        return Err("the engine is not running".into());
    }
    // SAFETY: the handle came from `nativeNew` and has not been freed; Kotlin frees it once, after its last call.
    let engine: &Handle = unsafe { &*(handle as *const Handle) };
    let mut guard = engine.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    catch_unwind(AssertUnwindSafe(|| f(&mut guard))).map_err(|_| "the engine failed unexpectedly".to_owned())
}

fn error(message: impl ToString) -> String {
    json!({ "error": message.to_string() }).to_string()
}

/// `config`, `run`, `import`, `sync`, `changes` and `api`, each with its argument as text; the answer is a JSON object.
pub fn call(engine: &mut Engine, op: &str, arg: &str) -> String {
    let done = |r: Result<(), String>| r.map_or_else(error, |()| "{}".into());
    match op {
        "config" => done(engine.set_config(arg)),
        // Which of the shared files the bucket replaced here, so the app can read them again.
        "sync" => engine
            .sync(arg)
            .map_or_else(error, |pulled| json!({ "settings": pulled }).to_string()),
        "run" => engine.run(arg),
        "import" => match serde_json::from_str::<Value>(arg) {
            Ok(v) => engine.import(
                v["text"].as_str().unwrap_or_default(),
                v["apply"].as_bool().unwrap_or(false),
                v["tz"].as_i64().unwrap_or(0) as i32,
                v["now"].as_f64().unwrap_or(0.0),
            ),
            Err(e) => error(format!("invalid request: {e}")),
        },
        "api" => match serde_json::from_str::<Value>(arg) {
            Ok(v) => engine.api(
                v["method"].as_str().unwrap_or("GET"),
                v["path"].as_str().unwrap_or_default(),
                v["body"].as_str().unwrap_or_default().as_bytes(),
            ),
            Err(e) => error(format!("invalid request: {e}")),
        },
        "changes" => json!({ "changes": engine.changes() }).to_string(),
        other => error(format!("unknown call: {other}")),
    }
}

#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeNew(
    mut env: JNIEnv,
    _class: JClass,
    dir: JString,
    key: JByteArray,
) -> jlong {
    let dir = text(&mut env, &dir);
    // The key that seals the hooks' secrets, kept by the app in the Android Keystore: exactly 32 bytes.
    let Ok(Ok(key)) = env.convert_byte_array(&key).map(<[u8; 32]>::try_from) else {
        return 0;
    };
    Box::into_raw(Box::new(Mutex::new(Engine::new(dir, &key)))) as jlong
}

#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeFree(_env: JNIEnv, _class: JClass, handle: jlong) {
    if handle != 0 {
        // SAFETY: see `with_engine`; this is the one place a handle is given back.
        drop(unsafe { Box::from_raw(handle as *mut Handle) });
    }
}

#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeCall(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    op: JString,
    arg: JString,
) -> jstring {
    let (op, arg) = (text(&mut env, &op), text(&mut env, &arg));
    let reply = with_engine(handle, |e| call(e, &op, &arg)).unwrap_or_else(error);
    answer(&env, reply)
}

/// The replica as bytes to keep, or null (and an `error` from the next `nativeCall`-free `nativeLastError`) if it
/// could not be saved.
#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeSave(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jbyteArray {
    match with_engine(handle, Engine::save) {
        Ok(Ok(bytes)) => env
            .byte_array_from_slice(&bytes)
            .map_or(std::ptr::null_mut(), |a| a.into_raw()),
        _ => std::ptr::null_mut(),
    }
}

/// Go on from saved bytes: null when it worked, the reason when it did not.
#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeRestore(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    bytes: JByteArray,
) -> jstring {
    let Ok(bytes) = env.convert_byte_array(&bytes) else {
        return answer(&env, "the saved copy could not be read");
    };
    match with_engine(handle, |e| e.restore(&bytes)) {
        Ok(Ok(())) => std::ptr::null_mut(),
        Ok(Err(e)) | Err(e) => answer(&env, e),
    }
}

// The three below take no handle and touch no engine: they are `crate::helpers`, so the screens can call them while a sync
// holds the engine.

/// The words of a command line as the engine reads them: a JSON array of strings.
#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeSplitWords(
    mut env: JNIEnv,
    _class: JClass,
    line: JString,
) -> jstring {
    let line = text(&mut env, &line);
    answer(&env, crate::helpers::words_json(&line))
}

/// A date in a `dateformat` pattern, in the zone `tz` seconds east of UTC.
#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeFormatDate(
    mut env: JNIEnv,
    _class: JClass,
    pattern: JString,
    epoch: jlong,
    tz: jint,
    week_starts_monday: jboolean,
) -> jstring {
    let pattern = text(&mut env, &pattern);
    answer(
        &env,
        crate::helpers::format_date(&pattern, epoch, tz, week_starts_monday != 0),
    )
}

/// Any other helper, by name, with its arguments as a JSON array (`tc_core::helpers::call`).
#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeHelper(
    mut env: JNIEnv,
    _class: JClass,
    name: JString,
    args: JString,
) -> jstring {
    let name = text(&mut env, &name);
    let args = text(&mut env, &args);
    answer(&env, crate::helpers::call(&name, &args))
}

/// A duration the way Taskwarrior writes it: `3d`, `2w`, `1.5y`.
#[no_mangle]
pub extern "system" fn Java_me_utsob_meghnaad_NativeEngine_nativeVague(
    _env: JNIEnv,
    _class: JClass,
    secs: jlong,
) -> jstring {
    answer(&_env, crate::helpers::vague(secs))
}
