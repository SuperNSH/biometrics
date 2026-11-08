//! Android: `BiometricManager` answers availability over JNI, and robius shows the prompt.
//!
//! NSH does not start `ndk-context`, which robius reads for its Java context. So this module finds
//! the VM through `libnativehelper` and the application through `ActivityThread`, and hands both to
//! `ndk-context` once. The manifest must declare `android.permission.USE_BIOMETRIC`; NSH 0.4.0 has no
//! setting that adds it, so the app adds it to the generated manifest (`docs/decisions.md`).

use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Mutex,
};

use jni::{
    Env, JavaVM, jni_sig, jni_str,
    objects::{JObject, JValue},
    sys::{jint, jsize},
};

use super::robius::{off_engine, start_prompt};
use crate::{
    backend::{Backend, BackendFuture, PromptDone},
    error::{BiometricsError, ErrorKind},
    options::{AuthOptions, BiometricType},
};

pub(crate) struct NativeBackend;

/// `BiometricManager.Authenticators.BIOMETRIC_STRONG` and `BIOMETRIC_WEAK`.
const BIOMETRIC_STRONG: jint = 0x000F;
const BIOMETRIC_WEAK: jint = 0x00FF;
/// `BiometricManager.BIOMETRIC_SUCCESS`.
const BIOMETRIC_SUCCESS: jint = 0;
/// `canAuthenticate(int)` exists from API 30; below it only the no-argument form does.
const SDK_WITH_AUTHENTICATORS: jint = 30;

/// `JNI_GetCreatedJavaVMs`, found by name in `libnativehelper`.
type GetCreatedVms = unsafe extern "C" fn(*mut *mut jni::sys::JavaVM, jsize, *mut jsize) -> jint;

/// Set once `ndk-context` holds the VM and the application. It can be set only once.
static CONTEXT: Mutex<bool> = Mutex::new(false);

impl From<jni::errors::Error> for BiometricsError {
    fn from(_: jni::errors::Error) -> Self {
        BiometricsError::new(ErrorKind::Backend)
    }
}

/// The VM of this process. An NSH app has no JVM of its own, so it is found through the runtime's library.
fn find_vm() -> Result<*mut jni::sys::JavaVM, BiometricsError> {
    // SAFETY: the names are valid C strings, and the symbol is called with the signature the JNI specification gives it.
    unsafe {
        let library = libc::dlopen(c"libnativehelper.so".as_ptr(), libc::RTLD_NOW);
        if library.is_null() {
            return Err(BiometricsError::new(ErrorKind::Backend));
        }
        let symbol = libc::dlsym(library, c"JNI_GetCreatedJavaVMs".as_ptr());
        if symbol.is_null() {
            return Err(BiometricsError::new(ErrorKind::Backend));
        }
        let get: GetCreatedVms = std::mem::transmute::<*mut c_void, GetCreatedVms>(symbol);
        let mut vm = std::ptr::null_mut();
        let mut count: jsize = 0;
        if get(&mut vm, 1, &mut count) != 0 || count < 1 || vm.is_null() {
            return Err(BiometricsError::new(ErrorKind::Backend));
        }
        Ok(vm)
    }
}

/// Runs `work` on this thread, attached to the VM.
fn attached<R>(
    work: impl FnOnce(&mut Env<'_>) -> Result<R, BiometricsError>,
) -> Result<R, BiometricsError> {
    let vm = find_vm()?;
    // SAFETY: the pointer came from the runtime's own `JNI_GetCreatedJavaVMs`, and the VM lives as long as the process.
    unsafe { JavaVM::from_raw(vm) }.attach_current_thread(work)
}

/// The application, which is a context for the framework's `BiometricPrompt` and `BiometricManager`.
/// Null before the runtime has created it.
fn application<'local>(env: &mut Env<'local>) -> Result<JObject<'local>, BiometricsError> {
    let application = env
        .call_static_method(
            jni_str!("android/app/ActivityThread"),
            jni_str!("currentApplication"),
            jni_sig!("()Landroid/app/Application;"),
            &[],
        )?
        .l()?;
    if application.is_null() {
        return Err(BiometricsError::new(ErrorKind::NoActivity));
    }
    Ok(application)
}

/// Gives `ndk-context` the VM and the application, once, so robius can reach Java. A context another
/// runtime installed already is adopted untouched.
fn ensure_context() -> Result<(), BiometricsError> {
    let mut ready = CONTEXT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if *ready {
        return Ok(());
    }
    // `ndk-context` has no try-get and asserts on a second initialization, and its getter panics
    // while nothing is installed, so the probe is the only way to tell an existing context apart.
    // The probe panics once on a fresh process; a silent hook keeps that out of the log.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let probe = std::panic::catch_unwind(AssertUnwindSafe(ndk_context::android_context));
    std::panic::set_hook(previous);
    if probe.is_ok() {
        *ready = true;
        return Ok(());
    }
    let vm = find_vm()?;
    let context = attached(|env| {
        let app = application(env)?;
        // The global reference lives as long as the process, which is what `ndk-context` assumes.
        let global = env.new_global_ref(app)?;
        Ok::<_, BiometricsError>(global.into_raw().cast::<c_void>())
    })?;
    // SAFETY: the VM and the application are the process's own, and the probe showed nothing is installed.
    unsafe { ndk_context::initialize_android_context(vm.cast(), context) };
    *ready = true;
    Ok(())
}

/// Runs `work` on the blocking pool, attached to the VM, and answers with its result.
async fn query<R: Send + 'static>(
    work: impl FnOnce(&mut Env<'_>) -> Result<R, BiometricsError> + Send + 'static,
) -> Result<R, BiometricsError> {
    off_engine(move || attached(work))
        .await
        .and_then(|answer| answer)
}

fn system_service<'local>(
    env: &mut Env<'local>,
    context: &JObject<'_>,
    name: &str,
) -> Result<JObject<'local>, BiometricsError> {
    let name: JObject<'_> = env.new_string(name)?.into();
    Ok(env
        .call_method(
            context,
            jni_str!("getSystemService"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
            &[JValue::Object(&name)],
        )?
        .l()?)
}

fn sdk_int(env: &mut Env<'_>) -> Result<jint, BiometricsError> {
    Ok(env
        .get_static_field(
            jni_str!("android/os/Build$VERSION"),
            jni_str!("SDK_INT"),
            jni_sig!("I"),
        )?
        .i()?)
}

/// `BiometricManager.canAuthenticate` for `authenticators`, or the no-argument form below API 30.
fn status(
    env: &mut Env<'_>,
    manager: &JObject<'_>,
    authenticators: jint,
) -> Result<jint, BiometricsError> {
    if sdk_int(env)? >= SDK_WITH_AUTHENTICATORS {
        Ok(env
            .call_method(
                manager,
                jni_str!("canAuthenticate"),
                jni_sig!("(I)I"),
                &[JValue::Int(authenticators)],
            )?
            .i()?)
    } else {
        Ok(env
            .call_method(manager, jni_str!("canAuthenticate"), jni_sig!("()I"), &[])?
            .i()?)
    }
}

/// Whether a biometric is enrolled and usable, by any class.
fn enrolled(env: &mut Env<'_>, context: &JObject<'_>) -> Result<bool, BiometricsError> {
    let manager = system_service(env, context, "biometric")?;
    Ok(status(env, &manager, BIOMETRIC_WEAK)? == BIOMETRIC_SUCCESS)
}

fn device_secure(env: &mut Env<'_>) -> Result<bool, BiometricsError> {
    let context = application(env)?;
    let keyguard = system_service(env, &context, "keyguard")?;
    Ok(env
        .call_method(&keyguard, jni_str!("isDeviceSecure"), jni_sig!("()Z"), &[])?
        .z()?)
}

fn can_check(env: &mut Env<'_>) -> Result<bool, BiometricsError> {
    let context = application(env)?;
    enrolled(env, &context)
}

fn biometric_types(env: &mut Env<'_>) -> Result<Vec<BiometricType>, BiometricsError> {
    let context = application(env)?;
    let manager = system_service(env, &context, "biometric")?;
    let mut types = Vec::new();
    if !enrolled(env, &context)? {
        return Ok(types);
    }
    if status(env, &manager, BIOMETRIC_STRONG)? == BIOMETRIC_SUCCESS {
        types.push(BiometricType::Strong);
    } else {
        types.push(BiometricType::Weak);
    }
    let package_manager = env
        .call_method(
            &context,
            jni_str!("getPackageManager"),
            jni_sig!("()Landroid/content/pm/PackageManager;"),
            &[],
        )?
        .l()?;
    let features = [
        ("android.hardware.fingerprint", BiometricType::Fingerprint),
        ("android.hardware.biometrics.face", BiometricType::Face),
        ("android.hardware.biometrics.iris", BiometricType::Iris),
    ];
    for (feature, kind) in features {
        let name: JObject<'_> = env.new_string(feature)?.into();
        let present = env
            .call_method(
                &package_manager,
                jni_str!("hasSystemFeature"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&name)],
            )?
            .z()?;
        if present {
            types.push(kind);
        }
    }
    Ok(types)
}

impl Backend for NativeBackend {
    fn is_device_supported(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(query(device_secure))
    }

    fn can_check_biometrics(&self) -> BackendFuture<'_, Result<bool, BiometricsError>> {
        Box::pin(query(can_check))
    }

    fn available_biometrics(
        &self,
    ) -> BackendFuture<'_, Result<Vec<BiometricType>, BiometricsError>> {
        Box::pin(query(biometric_types))
    }

    fn start_prompt(
        &self,
        reason: &str,
        options: &AuthOptions,
        done: PromptDone,
    ) -> Result<(), BiometricsError> {
        ensure_context()?;
        start_prompt(reason, options, !options.biometric_only, done)
    }

    fn cancel_prompt(&self) {}
}
