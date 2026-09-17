// Only loaded by Vite's explicit test mode; no filesystem operations.
import { mockIPC } from "@tauri-apps/api/mocks";
const entries = Array.from({ length: 3000 }, (_, i) => ({
  relative: `Series/Collection ${Math.floor(i / 20)}/Episode ${i}.mkv`,
  target: `W:/Shows/Collection ${Math.floor(i / 20)}/Episode ${i}.mkv`,
  status: i % 10 === 0 ? "Broken" : i % 37 === 0 ? "Unreadable" : "OK",
  link_is_dir: false,
}));
const data = { src_root: "D:/Media/Library", entries };
mockIPC(async (command, args = {}) => {
  const payload = /** @type {any} */ (args);
  if (command === "plugin:dialog|open")
    return payload.options?.directory ? "D:/Media/Library" : "snapshot.json";
  if (command === "plugin:dialog|save") return "symlinks.json";
  if (command === "scan_symlinks") {
    await new Promise((resolve) => setTimeout(resolve, 350));
    return { ...data, skipped: 2 };
  }
  if (command === "load_export") return data;
  if (command === "export_symlinks") {
    if (!payload.data.entries.length) throw new Error("Empty export");
    return;
  }
  if (command === "preview_recreate") {
    const rule = payload.mappings[0];
    return {
      roots: [{ root: "W:/Shows", count: 3000 }],
      sample: [],
      root_samples: [
        {
          root: "W:/Shows",
          link: payload.dstRoot + "/" + entries[0].relative,
          target: rule
            ? entries[0].target.replace(
                rule.from.replaceAll("\\", "/"),
                rule.to,
              )
            : entries[0].target,
        },
      ],
    };
  }
  if (command === "check_recreate_conflicts")
    return {
      total: 2,
      non_symlink: 1,
      sample: ["D:/Media/Library/existing.mkv"],
    };
  if (command === "recreate_symlinks")
    return {
      created: 2999,
      failed: ["Fixture: permission denied for one link"],
      sample_links: ["D:/Media/Library/Series/Collection 0/Episode 0.mkv"],
    };
  throw new Error("Unexpected fixture command: " + command);
});
