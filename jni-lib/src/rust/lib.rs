extern crate jni;
extern crate aurora_client;

use jni::JNIEnv;
use jni::objects::JClass;

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn Java_uk_me_rochester_Aurora_runService(env: JNIEnv, _class: JClass) {
    //Note that this shouldn't panic in prod, but we catch any panic anyway
    //so that we don't mess with java by unwinding through the JVM which
    //isn't build to support rust unwinds
    let res = std::panic::catch_unwind(|| { aurora_client::run_service() });
    match res {
        Ok(res) => {
            match res {
                Ok(_) => (),
                Err(e) => {
                    let _ = env.throw_new("java/lang/Exception", format!("Rust error: {:?}", e));
                }
            }
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/Exception", format!("Rust panic: {:?}", e));
        }
    };
}
