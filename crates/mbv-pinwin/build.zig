const std = @import("std");

// Builds pinwin (a pinned Zig dependency, see build.zig.zon) and installs its static
// archives under `lib/` for build.rs to link. A Zig static library does not bundle the
// archives it links, so every static dependency of `pinwin` is installed beside it.
pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const pinwin = b.dependency("pinwin", .{ .target = target, .optimize = optimize });
    for (pinwin.artifact("pinwin").getCompileDependencies(false)) |archive| {
        if (archive.kind == .lib and archive.linkage == .static) b.installArtifact(archive);
    }
}
