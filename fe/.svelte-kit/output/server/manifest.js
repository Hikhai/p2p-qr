export const manifest = (() => {
function __memo(fn) {
	let value;
	return () => value ??= (value = fn());
}

return {
	appDir: "_app",
	appPath: "_app",
	assets: new Set(["favicon.svg"]),
	mimeTypes: {".svg":"image/svg+xml"},
	_: {
		client: {start:"_app/immutable/entry/start.CwQHLqCG.js",app:"_app/immutable/entry/app._2PRFYFB.js",imports:["_app/immutable/entry/start.CwQHLqCG.js","_app/immutable/chunks/Bp8SEVUB.js","_app/immutable/chunks/Bl5Hxi-C.js","_app/immutable/chunks/CcB5Wpuy.js","_app/immutable/entry/app._2PRFYFB.js","_app/immutable/chunks/CcB5Wpuy.js","_app/immutable/chunks/Bl5Hxi-C.js","_app/immutable/chunks/Bzak7iHL.js","_app/immutable/chunks/K5nnekEa.js"],stylesheets:[],fonts:[],uses_env_dynamic_public:false},
		nodes: [
			__memo(() => import('./nodes/0.js')),
			__memo(() => import('./nodes/1.js')),
			__memo(() => import('./nodes/2.js'))
		],
		remotes: {
			
		},
		routes: [
			{
				id: "/",
				pattern: /^\/$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 2 },
				endpoint: null
			}
		],
		prerendered_routes: new Set([]),
		matchers: async () => {
			
			return {  };
		},
		server_assets: {}
	}
}
})();
