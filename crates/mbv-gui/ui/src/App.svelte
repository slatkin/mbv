<script lang="ts">
  // The webview's own context menu is turned off (add-tauri-gui-client D11).
  // Tauri has no config key for it on WebKitGTK.
  function preventContextMenu(event: MouseEvent) {
    event.preventDefault();
  }
</script>

<svelte:document oncontextmenu={preventContextMenu} />

<main class="h-dvh bg-background text-foreground"></main>
