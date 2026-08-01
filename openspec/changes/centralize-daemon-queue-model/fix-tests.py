DTEST = "/home/slatkin/Dev/mbv/.worktrees/centralize-daemon-queue-model/crates/mbv-core/src/daemon_tests.rs"
with open(DTEST) as f:
    dt = f.read()

# Count issues
print(f"Before: items refs={dt.count('shared_queue.items')}, cursor refs={dt.count('shared_queue.cursor')}")
print(f"connect_client calls: {dt.count('connect_client(&mut clients)')}")

# 1. connect_client needs peer_version param now
# The helper function calls `clients.connect(tx)` - needs `clients.connect(tx, 0u32)` or similar
dt = dt.replace(
    "let id = clients.connect(tx);",
    "let id = clients.connect(tx, 7);  // v7 peer in tests"
)

# 2. Remove items/cursor from handle_ctrl calls - more patterns
# The tests use indentation patterns like:
#   &client,
#   &player,
#   false,
#   &mut items,
#   &mut cursor,
#   &mut source,
# These become:
#   &client,
#   &player,
#   false,
#   &mut source,
# Let me target ALL test handle_ctrl calls

# Pattern: remove all "&mut items,\n<any whitespace>&mut cursor,\n"
import re
# Remove &mut items, followed by whitespace+&mut cursor, followed by newline
dt = re.sub(r'&mut items,\n(\s+)&mut cursor,\n', '', dt)

# Handle leftover: "&mut items, &mut cursor," (single line, already handled)
dt = dt.replace('&mut items, &mut cursor,', '')

# 3. Fix shared_queue.items -> shared_queue.queue (remaining)
dt = dt.replace('shared_queue.items', 'shared_queue.queue')

# 4. Fix all *shared_queue.cursor occurrences
# These are in assertions or readings
dt = dt.replace('*shared_queue.cursor.lock().unwrap()', 'shared_queue.queue.lock().unwrap().current_index().unwrap_or(0)')

# 5. Fix handle_ws calls that pass items/cursor
# handle_ws is called like:
# handle_ws(event, &client, &player, false, &mut items, &mut cursor, &mut source, &shared_queue, &registry);
# New sig (after 2.x): handle_ws(event, &client, &player, false, &mut source, &shared_queue, &registry);
# Remove &mut items and &mut cursor params
# Already handled by the regex above

# 6. Remove test-local declarations that are now unused
# Tests declare: let mut items = vec![]; let mut cursor = 0;
# These need removing or their uses need changing
# Remove lines like: let mut items = ...; 
# But keep them if they're used for assertions

# 7. Fix any test assertion that reads items or cursor from shared_queue 
# Already handled by the shared_queue replacements above

# 8. Fix adopt_queue test: it reads items from standalone queue var
# The test constructs items/cursor locally and passes them

with open(DTEST, "w") as f:
    f.write(dt)

print(f"After: items refs={dt.count('shared_queue.items')}, cursor refs={dt.count('shared_queue.cursor')}")
print(f"connect_client calls: {dt.count('connect_client(&mut clients)')}")
print("Test fixes applied")
