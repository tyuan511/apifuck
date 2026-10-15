import {resolve} from 'node:path';
const root=import.meta.dir;
const result=await Bun.build({entrypoints:[resolve(root,'bridge.ts')],outdir:root,naming:'bridge.bundle.js',target:'browser',format:'iife',minify:true,plugins:[{name:'native-conversion-boundary',setup(build){
  build.onResolve({filter:/^@usebruno\/lang$/},()=>({path:resolve(root,'lang-facade.ts')}));
  build.onResolve({filter:/^@usebruno\/common$/},()=>({path:resolve(root,'common-facade.ts')}));
  build.onResolve({filter:/^@usebruno\/common\/utils$/},()=>({path:resolve(root,'utils-facade.ts')}));
  build.onResolve({filter:/^nanoid$/},()=>({path:resolve(root,'node_modules/nanoid/non-secure/index.js')}));
}}]});
if(!result.success){for(const log of result.logs)console.error(log);process.exit(1);}

const runtime=await Bun.build({entrypoints:[resolve(root,"runtime.ts")],outdir:root,naming:"runtime.bundle.js",target:"browser",format:"iife",minify:true});if(!runtime.success){console.error(runtime.logs);process.exit(1);}
