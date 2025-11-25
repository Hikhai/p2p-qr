

export const index = 2;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/_page.svelte.js')).default;
export const universal = {
  "ssr": false,
  "csr": true
};
export const universal_id = "src/routes/+page.ts";
export const imports = ["_app/immutable/nodes/2.xmsCqsf7.js","_app/immutable/chunks/Bzak7iHL.js","_app/immutable/chunks/DdeGdQev.js","_app/immutable/chunks/CcB5Wpuy.js","_app/immutable/chunks/Bl5Hxi-C.js","_app/immutable/chunks/K5nnekEa.js"];
export const stylesheets = ["_app/immutable/assets/2.cWnJbwe1.css"];
export const fonts = [];
