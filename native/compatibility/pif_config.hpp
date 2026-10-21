// Butterd's reduced PIF configuration. GPL-3.0-only; see THIRD_PARTY_NOTICES.txt.
#pragma once
#include <string>
#include <string_view>
#include <unordered_map>
namespace pif {
struct Config {
    std::unordered_map<std::string, std::string> propMap;
    bool spoofBuild = true;
    bool spoofVendingBuild = true;
    bool spoofProps = true;
    static constexpr bool debug = false;
    std::string buildId, securityPatch;
};
Config parseConfig(std::string_view content);
}
