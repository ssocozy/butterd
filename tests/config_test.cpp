#include "pif_config.hpp"
#include <cassert>
#include <string>
int main() {
    auto defaults = pif::parseConfig("");
    assert(defaults.spoofBuild && defaults.spoofProps && defaults.spoofVendingBuild);
    auto config = pif::parseConfig("spoofBuild=false\nspoofProps=0\nspoofVendingBuild=false\nspoofSignature=true\nSDK_INT=25\nDEVICE_INITIAL_SDK_INT=25\n");
    assert(!config.spoofBuild && !config.spoofProps && !config.spoofVendingBuild);
    assert(config.propMap.empty());
    config = pif::parseConfig("FINGERPRINT=brand/product/device:14/BUILD/123:user/release-keys\nMODEL=Example\n");
    assert(config.propMap.at("DEVICE") == "device");
    assert(config.propMap.at("ID") == "BUILD");
    assert(config.propMap.at("MODEL") == "Example");
    assert(pif::parseConfig("FINGERPRINT=broken\nMODEL=Example").propMap.empty());
    assert(pif::parseConfig(std::string(17000, 'x')).propMap.empty());
    assert(pif::parseConfig("MODEL=" + std::string(300, 'x')).propMap.empty());
    assert(pif::parseConfig("# comment\r\n MODEL = Example # note\n").propMap.at("MODEL") == "Example");
}
