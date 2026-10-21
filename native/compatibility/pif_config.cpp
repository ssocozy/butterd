// Reduced from KOWX712/PlayIntegrityFix config parsing. Modified for Butterd.
// SPDX-License-Identifier: GPL-3.0-only
#include "pif_config.hpp"
#include <array>
namespace pif {
namespace {
std::string trim(std::string_view value) {
    auto start = value.find_first_not_of(" \t\r");
    if (start == std::string_view::npos) return {};
    return std::string(value.substr(start, value.find_last_not_of(" \t\r") - start + 1));
}
bool allowed(std::string_view key) {
    constexpr std::array keys = {"BRAND", "MANUFACTURER", "MODEL", "DEVICE", "PRODUCT", "FINGERPRINT",
                                "ID", "INCREMENTAL", "RELEASE", "SECURITY_PATCH", "TYPE", "TAGS"};
    for (auto item : keys) if (key == item) return true;
    return false;
}
}
Config parseConfig(std::string_view text) {
    Config config;
    if (text.size() > 16384) return config;
    while (!text.empty()) {
        auto end = text.find('\n');
        auto line = text.substr(0, end);
        auto comment = line.find('#');
        line = line.substr(0, comment);
        auto equal = line.find('=');
        if (equal != std::string_view::npos) {
            auto key = trim(line.substr(0, equal)), value = trim(line.substr(equal + 1));
            bool valid = value.size() <= 256;
            for (unsigned char ch : value) if (ch < 32 || ch > 126) valid = false;
            if (valid) {
                bool* flag = key == "spoofBuild" ? &config.spoofBuild : key == "spoofProps" ? &config.spoofProps
                             : key == "spoofVendingBuild" ? &config.spoofVendingBuild : nullptr;
                if (flag) *flag = value == "true" || value == "1";
                else if (allowed(key) && !value.empty()) config.propMap[key] = value;
            }
        }
        if (end == std::string_view::npos) break;
        text.remove_prefix(end + 1);
    }
    if (auto it = config.propMap.find("FINGERPRINT"); it != config.propMap.end()) {
        std::string_view fingerprint(it->second);
        constexpr std::array fields = {"BRAND", "PRODUCT", "DEVICE", "RELEASE", "ID", "INCREMENTAL", "TYPE", "TAGS"};
        std::unordered_map<std::string, std::string> expanded;
        for (auto field : fields) {
            auto delimiter = fingerprint.find_first_of("/:");
            expanded[field] = std::string(fingerprint.substr(0, delimiter));
            if (delimiter == std::string_view::npos) fingerprint = {};
            else fingerprint.remove_prefix(delimiter + 1);
        }
        bool complete = true;
        for (const auto& item : expanded) if (item.second.empty()) complete = false;
        if (complete) for (const auto& [key, value] : expanded) config.propMap.try_emplace(key, value);
        else config.propMap.clear(); // An incomplete identity must never be partially applied.
    }
    if (auto it = config.propMap.find("ID"); it != config.propMap.end()) config.buildId = it->second;
    if (auto it = config.propMap.find("SECURITY_PATCH"); it != config.propMap.end()) config.securityPatch = it->second;
    return config;
}
}
