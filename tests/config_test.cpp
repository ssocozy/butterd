#include "pif_config.hpp"
#include <cassert>
#include <string>
int main() {
    auto defaults = pif::parseConfig("");
    assert(defaults.spoofBuild && defaults.spoofProps && defaults.spoofVendingBuild);
    auto config = pif::parseConfig("spoofBuild=false\nspoofProps=0\nspoofVendingBuild=false\nspoofSignature=true\nSDK_INT=25\nDEVICE_INITIAL_SDK_INT=25\n");
    assert(!config.spoofBuild && !config.spoofProps && !config.spoofVendingBuild);
    assert(config.propMap.empty());
    assert(config.deviceInitialSdkInt == "25");
    assert(pif::parseConfig("DEVICE_INITIAL_SDK_INT=invalid\n").deviceInitialSdkInt.empty());
    assert(pif::parseConfig("DEVICE_INITIAL_SDK_INT=999\n").deviceInitialSdkInt.empty());
    assert(pif::parseConfig("SDK_INT=25\n").propMap.empty());
    auto metadata = pif::parseConfig("SECURITY_PATCH=2026-10-05\nDEVICE_INITIAL_SDK_INT=35\n");
    assert(metadata.securityPatch == "2026-10-05" && metadata.deviceInitialSdkInt == "35");
    assert(metadata.propMap.count("DEVICE_INITIAL_SDK_INT") == 0);
    config = pif::parseConfig("FINGERPRINT=brand/product/device:14/BUILD/123:user/release-keys\nMODEL=Example\n");
    assert(config.propMap.at("DEVICE") == "device");
    assert(config.propMap.at("ID") == "BUILD");
    assert(config.propMap.at("MODEL") == "Example");
    assert(pif::parseConfig("FINGERPRINT=broken\nMODEL=Example").propMap.empty());
    assert(pif::parseConfig(std::string(17000, 'x')).propMap.empty());
    assert(pif::parseConfig("MODEL=" + std::string(300, 'x')).propMap.empty());
    assert(pif::parseConfig("# comment\r\n MODEL = Example # note\n").propMap.at("MODEL") == "Example");
}
