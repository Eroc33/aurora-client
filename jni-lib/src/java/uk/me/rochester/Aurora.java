package uk.me.rochester;

class Aurora {
    public static native void runService();

    static {
        // This actually loads the shared object that we'll be creating.
        // The actual location of the .so or .dll may differ based on your
        // platform.
        System.loadLibrary("aurora_client_jni");
    }

    public static void main(String[] args){
        runService();
    }
}