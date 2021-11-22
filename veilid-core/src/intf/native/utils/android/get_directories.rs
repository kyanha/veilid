use super::*;
use crate::xx::*;

pub fn get_files_dir() -> String {
    let aglock = ANDROID_GLOBALS.lock();
    let ag = aglock.as_ref().unwrap();
    let env = ag.vm.attach_current_thread().unwrap();
    // context.getFilesDir().getAbsolutePath()
    let file = env
        .call_method(ag.ctx.as_obj(), "getFilesDir", "()Ljava/io/File;", &[])
        .unwrap()
        .l()
        .unwrap();
    let path = env
        .call_method(file, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .unwrap()
        .l()
        .unwrap();

    let jstrval = env.get_string(JString::from(path)).unwrap();
    String::from(jstrval.to_string_lossy())
}

pub fn get_cache_dir() -> String {
    let aglock = ANDROID_GLOBALS.lock();
    let ag = aglock.as_ref().unwrap();
    let env = ag.vm.attach_current_thread().unwrap();
    // context.getCacheDir().getAbsolutePath()
    let file = env
        .call_method(ag.ctx.as_obj(), "getCacheDir", "()Ljava/io/File;", &[])
        .unwrap()
        .l()
        .unwrap();
    let path = env
        .call_method(file, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .unwrap()
        .l()
        .unwrap();

    let jstrval = env.get_string(JString::from(path)).unwrap();
    String::from(jstrval.to_string_lossy())
}
