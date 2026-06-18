# cortex-android

Android companion app for the TPT Cortex scripting runtime.

## Setup

### Gradle Wrapper

The `gradle-wrapper.jar` binary is not included in version control. Before building, generate it by running:

```sh
gradle wrapper --gradle-version=8.7
```

This requires Gradle to be installed on your system. After running it, `gradle/wrapper/gradle-wrapper.jar` will be created and `./gradlew` will work normally.

### Build

```sh
./gradlew assembleDebug
```

## Architecture

- **WebSocket Server**: Binds to `127.0.0.1:9911`, same port as the Go desktop daemon, so the PWA (`CortexClient.ts`) connects without code changes.
- **Script Engine**: Rust JNI library (`libcortex_engine.so`) compiles scripts to JSON AST; `CortexInterpreter.kt` walks the AST.
- **Native APIs**: `AndroidNativeRegistry` dispatches `native.log`, `native.db`, `native.http`, `native.location` to Android APIs.
- **Foreground Service**: `CortexForegroundService` holds a persistent notification and keeps the WebSocket server alive.
- **Room DB**: Stores script-written records in `records` table (tableName + JSON data).
- **Location**: `LocationRepository` wraps `FusedLocationProviderClient` with a `StateFlow<Location?>`.
