// Native build-field path adapted from KOWX712/PlayIntegrityFix.
// Modified for Butterd: three options, bounded configuration, no dex payload.
// SPDX-License-Identifier: GPL-3.0-only
#include "zygisk.hpp"
#include "pif_config.hpp"
#include "property_virtualizer.hpp"
#include <android/log.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <unistd.h>
#include <cstdint>
#include <string>

namespace {
constexpr uint32_t maximum = 16384;
constexpr auto configPath = "/data/adb/modules/butterd/compatibility/pif.prop";
bool transfer(int fd, void* memory, size_t size, bool writing) {
    auto* cursor = static_cast<char*>(memory);
    while (size) {
        auto count = writing ? TEMP_FAILURE_RETRY(write(fd, cursor, size)) : TEMP_FAILURE_RETRY(read(fd, cursor, size));
        if (count <= 0) return false;
        cursor += count; size -= static_cast<size_t>(count);
    }
    return true;
}
void deadline(int fd) {
    const timeval timeout{.tv_sec = 3, .tv_usec = 0};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
    setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout));
}
void companion(int fd) {
    deadline(fd);
    int file = open(configPath, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
    struct stat info{};
    uint32_t size = 0;
    std::string text;
    if (file >= 0 && fstat(file, &info) == 0 && S_ISREG(info.st_mode) && info.st_uid == 0
            && info.st_size > 0 && info.st_size <= maximum) {
        text.resize(static_cast<size_t>(info.st_size));
        if (transfer(file, text.data(), text.size(), false)) size = static_cast<uint32_t>(text.size());
    }
    if (file >= 0) close(file);
    if (transfer(fd, &size, sizeof(size), true) && size) transfer(fd, text.data(), size, true);
}
size_t updateBuild(JNIEnv* env, const pif::Config& config) {
    size_t applied = 0;
    jclass build = env->FindClass("android/os/Build");
    if (!build) { env->ExceptionClear(); return 0; }
    jclass version = env->FindClass("android/os/Build$VERSION");
    if (!version) { env->ExceptionClear(); env->DeleteLocalRef(build); return 0; }
    for (const auto& [key, value] : config.propMap) {
        auto target = build;
        auto field = env->GetStaticFieldID(build, key.c_str(), "Ljava/lang/String;");
        if (!field) {
            env->ExceptionClear(); target = version;
            field = env->GetStaticFieldID(version, key.c_str(), "Ljava/lang/String;");
        }
        if (!field) { env->ExceptionClear(); continue; }
        auto text = env->NewStringUTF(value.c_str());
        if (!text) { env->ExceptionClear(); continue; }
        env->SetStaticObjectField(target, field, text);
        bool verified = false;
        if (env->ExceptionCheck()) env->ExceptionClear();
        else {
            auto observed = static_cast<jstring>(env->GetStaticObjectField(target, field));
            if (env->ExceptionCheck()) env->ExceptionClear();
            else if (observed) {
                const char* actual = env->GetStringUTFChars(observed, nullptr);
                if (actual) { verified = value == actual; env->ReleaseStringUTFChars(observed, actual); }
                else env->ExceptionClear();
            }
            if (observed) env->DeleteLocalRef(observed);
        }
        if (verified) ++applied;
        else __android_log_print(ANDROID_LOG_WARN, "butter", "PIF build field readback failed: %s", key.c_str());
        env->DeleteLocalRef(text);
    }
    env->DeleteLocalRef(version); env->DeleteLocalRef(build);
    return applied;
}
class ButterdCompatibility : public zygisk::ModuleBase {
    zygisk::Api* api = nullptr;
    JNIEnv* env = nullptr;
    pif::Config config;
    bool gms = false, store = false, loaded = false, propsApplied = false;
public:
    void onLoad(zygisk::Api* interface, JNIEnv* jni) override { api = interface; env = jni; }
    void preAppSpecialize(zygisk::AppSpecializeArgs* args) override {
        gms = store = loaded = propsApplied = false; config = {};
        if (!args || !args->nice_name || !args->app_data_dir) { api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY); return; }
        const char* rawName = env->GetStringUTFChars(args->nice_name, nullptr);
        if (!rawName) { env->ExceptionClear(); api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY); return; }
        std::string name(rawName); env->ReleaseStringUTFChars(args->nice_name, rawName);
        const char* rawDir = env->GetStringUTFChars(args->app_data_dir, nullptr);
        if (!rawDir) { env->ExceptionClear(); api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY); return; }
        std::string dir(rawDir); env->ReleaseStringUTFChars(args->app_data_dir, rawDir);
        gms = name == "com.google.android.gms.unstable" && dir.ends_with("/com.google.android.gms");
        store = name == "com.android.vending" && dir.ends_with("/com.android.vending");
        if (!gms && !store) { api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY); return; }
        api->setOption(zygisk::FORCE_DENYLIST_UNMOUNT);
        int fd = api->connectCompanion();
        if (fd < 0) { __android_log_print(ANDROID_LOG_ERROR, "butter", "PIF companion connection failed"); api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY); return; }
        deadline(fd);
        uint32_t size = 0;
        std::string text;
        bool ok = transfer(fd, &size, sizeof(size), false) && size > 0 && size <= maximum;
        if (ok) { text.resize(size); ok = transfer(fd, text.data(), size, false); }
        close(fd);
        if (ok) config = pif::parseConfig(text);
        loaded = ok && !config.propMap.empty();
        if (!loaded) { __android_log_print(ANDROID_LOG_ERROR, "butter", "PIF identity missing or invalid; no spoof applied"); api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY); return; }
        if (gms && config.spoofProps) {
            propsApplied = pif::property_virtualizer::install(config);
            if (!propsApplied) __android_log_print(ANDROID_LOG_WARN, "butter", "PIF scoped properties unavailable; skipped");
        }
    }
    void postAppSpecialize(const zygisk::AppSpecializeArgs*) override {
        if (!loaded) return;
        const bool buildRequested = (gms && config.spoofBuild) || (store && config.spoofVendingBuild);
        const size_t fields = buildRequested ? updateBuild(env, config) : 0;
        __android_log_print(ANDROID_LOG_INFO, "butter", "PIF process=%s buildRequested=%d fieldsVerified=%zu propsApplied=%d", gms ? "gms.unstable" : "vending", buildRequested, fields, propsApplied);
    }
    void preServerSpecialize(zygisk::ServerSpecializeArgs*) override { api->setOption(zygisk::DLCLOSE_MODULE_LIBRARY); }
};
}
REGISTER_ZYGISK_MODULE(ButterdCompatibility)
REGISTER_ZYGISK_COMPANION(companion)
