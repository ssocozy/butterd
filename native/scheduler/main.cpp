// Narrow root helper: change only SurfaceFlinger thread uclamp.min and cpuset.
#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <sys/syscall.h>
#include <unistd.h>

struct Attributes {
    uint32_t size, policy;
    uint64_t flags;
    int32_t nice;
    uint32_t priority;
    uint64_t runtime, deadline, period;
    uint32_t minimum, maximum;
};
static_assert(sizeof(Attributes) == 56, "Unexpected sched_attr layout");
static std::string read(const std::string& path) {
    std::ifstream file(path);
    if (!file) throw std::runtime_error("Cannot read thread state");
    std::ostringstream text; text << file.rdbuf();
    if (text.str().size() > 16384) throw std::runtime_error("Thread state exceeds limit");
    return text.str();
}
static unsigned long number(const std::string& text) {
    if (text.empty() || text.find_first_not_of("0123456789") != std::string::npos)
        throw std::runtime_error("Invalid numeric argument");
    return std::stoul(text);
}
static bool group(const std::string& value) {
    return value == "/" || value == "/foreground" || value == "/system-background" ||
        value == "/top-app" || value == "/background";
}
int main(int argc, char** argv) {
    try {
        if (getuid() != 0 || (argc != 3 && argc != 6)) throw std::runtime_error("Invalid scheduler command");
        const auto tid = number(argv[2]);
        if (tid == 0 || tid > 4194304) throw std::runtime_error("Invalid thread ID");
        const std::string base = "/proc/" + std::to_string(tid);
        const auto status = read(base + "/status");
        const auto offset = status.find("Tgid:");
        if (offset == std::string::npos) throw std::runtime_error("Missing process identity");
        std::istringstream ids(status.substr(offset + 5)); unsigned long pid = 0; ids >> pid;
        if (read("/proc/" + std::to_string(pid) + "/comm") != "surfaceflinger\n")
            throw std::runtime_error("Only SurfaceFlinger threads are supported");
        const auto stat = read(base + "/stat");
        const auto end = stat.rfind(')');
        if (end == std::string::npos) throw std::runtime_error("Missing thread identity");
        std::istringstream fields(stat.substr(end + 2)); std::string start;
        for (int field = 3; field <= 22; ++field) if (!(fields >> start)) throw std::runtime_error("Invalid thread identity");
        Attributes attributes{}; attributes.size = sizeof(attributes);
        if (syscall(SYS_sched_getattr, tid, &attributes, sizeof(attributes), 0) != 0)
            throw std::runtime_error("Cannot read scheduler attributes");
        auto cpuset = read(base + "/cpuset");
        if (!cpuset.empty() && cpuset.back() == '\n') cpuset.pop_back();
        if (!group(cpuset)) throw std::runtime_error("Unsupported compositor cpuset");
        if (std::string(argv[1]) == "get" && argc == 3) {
            std::cout << start << ' ' << attributes.minimum << ' ' << attributes.maximum << ' ' << cpuset << '\n';
        } else if (std::string(argv[1]) == "set" && argc == 6) {
            const auto minimum = number(argv[4]); const std::string target = argv[5];
            if (start != argv[3] || minimum > 1024 || minimum > attributes.maximum || !group(target))
                throw std::runtime_error("Thread identity or target changed");
            if (attributes.minimum != minimum) {
                // KEEP_POLICY | KEEP_PARAMS | UTIL_CLAMP_MIN. Preserve uclamp.max.
                attributes.flags = 8 | 16 | 32; attributes.minimum = minimum;
                if (syscall(SYS_sched_setattr, tid, &attributes, 0) != 0)
                    throw std::runtime_error("Cannot set compositor minimum clamp");
            }
            if (cpuset != target) {
                std::ofstream tasks("/dev/cpuset" + (target == "/" ? std::string() : target) + "/tasks");
                tasks << tid; tasks.flush();
                if (!tasks) throw std::runtime_error("Cannot move compositor thread");
            }
        } else throw std::runtime_error("Invalid scheduler command");
        return 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << " (errno " << errno << ")\n"; return 1;
    }
}
