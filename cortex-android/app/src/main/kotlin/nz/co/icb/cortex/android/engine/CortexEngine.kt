package nz.co.icb.cortex.android.engine

object CortexEngine {
    init {
        System.loadLibrary("cortex_engine")
    }

    /**
     * Compiles a Cortex script source into a JSON AST.
     * @param source The script source code
     * @param allow Comma-separated list of allowed native API namespaces
     * @return JSON string representing the AST, or a JSON error object
     */
    external fun nativeCompile(source: String, allow: String): String
}
