

export const index = 0;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/fallbacks/layout.svelte.js')).default;
export const universal = {
  "ssr": false
};
export const universal_id = "src/routes/+layout.ts";
export const imports = ["_app/immutable/nodes/0.CTFtfWYy.js","_app/immutable/chunks/Bzak7iHL.js","_app/immutable/chunks/CcB5Wpuy.js"];
export const stylesheets = [];
export const fonts = [];
