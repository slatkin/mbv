# Measurements

This file holds the measurement protocol for the Blitz Client spike (design D16). The run is not done. Each live run against the user's Owner needs the user's approval (Open Question 2). Do not run these commands until the user approves.

Use the commands as written. Replace `<pid>` with the process ID from the first step. Write no script.

## Protocol

### 1. Process IDs

```
pgrep -a mbv-native
pgrep -a mbv-gui
pgrep -a WebKitWebProcess
pgrep -a WebKitNetworkProcess
```

### 2. PSS and anonymous PSS

Run this for each process ID.

```
rg '^(Pss|Pss_Anon):' /proc/<pid>/smaps_rollup
```

### 3. GPU driver share

This step lists the mappings whose path names a driver library: `libvulkan`, `libGL`, `libEGL`, `libgallium`, or an NVIDIA driver library. Sum the `Pss:` lines of those mappings by hand. Record the sum on its own line in the results.

```
rg -A 6 -e 'libvulkan' -e 'libGL' -e 'libEGL' -e 'libgallium' -e 'nvidia' /proc/<pid>/smaps
```

### 4. Threads

```
rg '^Threads:' /proc/<pid>/status
ps -L -p <pid> | wc -l
```

The `ps` count includes the header line. Subtract 1 from it.

### 5. CPU

Read the stat fields at the start and after 300 s.

```
cut -d' ' -f14,15 /proc/<pid>/stat
sleep 300
cut -d' ' -f14,15 /proc/<pid>/stat
getconf CLK_TCK
```

The fields are utime and stime, in clock ticks. Average CPU is `((utime2 + stime2) - (utime1 + stime1)) / (300 × CLK_TCK)`. Multiply by 100 for a percentage of one core. The process names have no spaces, so fields 14 and 15 keep their numbers.

### 6. Heap (profiling build)

```
cargo build -p mbv-native --features profiling
heaptrack ./target/debug/mbv-native
heaptrack_print <heaptrack data file>
```

Read the peak heap from `heaptrack_print`. Design D16 names `./target/profiling/mbv-native`. The workspace has no Cargo profile named `profiling`, so this file uses the debug build path. Resolve the path before the run.

## GUI thread-count baseline

Run the protocol against the GUI on the same machine, with the same Owner and the same item, before the spike run. The total is the sum of the GUI rows.

| Process | Threads (`/proc/<pid>/status`) | Threads (`ps -L`, minus header) | Notes |
| --- | --- | --- | --- |
| `mbv-gui` | | | |
| `WebKitWebProcess` | | | |
| `WebKitNetworkProcess` | | | |
| Total (GUI) | | | |

## Go / No-Go

The verdict is **go** when G-Mem, G-Heap, G-Threads, G-CPU, G-Playhead, and G-Crash pass, and the user accepts G-Look. Any other result is **no-go**.

| Gate | Bound | Result | Pass / Fail |
| --- | --- | --- | --- |
| G-Mem | Total PSS of the `mbv-native` process, with the GPU driver share, at idle with playback: at most 250 MB | | |
| G-Heap | Profiling build heaptrack peak: at most 30 MB | | |
| G-Threads | Total thread count: at most the GUI's total under the same protocol | | |
| G-CPU | Average CPU over 5 minutes of playback: at most 5 % of one core, with the playhead written at 4 Hz | | |
| G-Playhead | While playing, the playhead is written at least every 250 ms. A seek or a pause takes effect within one tick. The position error against the Owner is at most 1 s. No write occurs while paused. | | |
| G-Crash | 30 minutes of playback with track changes and window resizes: no panic and no GPU surface timeout exit, on a host with a GPU | | |
| G-Look | The user judges whether the Queue panel matches the GUI's layout and colours. Differences are limited to the absent or open gaps G1, G2, and G10. | | |

## Results

