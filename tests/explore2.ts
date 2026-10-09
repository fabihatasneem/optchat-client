import { BUNDLE, Cua } from "./lib/cua.ts";
const c = await Cua.start();
let r = await c.js(`globalThis.app = await cua.getApp("${BUNDLE}"); 1`);
r = await c.js(`const s = await app.getAXState({disableDiffing:true, emit:false}); nodeRepl.write(s.slice(0, 3000))`);
console.log(r.text);
c.close();
