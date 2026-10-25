// Run the actual browser suites in an owned Chrome process, on isolated ports.
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const args=process.argv.slice(2),option=(key,fallback)=>args.includes(key)?args[args.indexOf(key)+1]:fallback;
const out=path.resolve(option('--out','D:/BuildCache/glory-bulk-optimization/browser-validation'));
assert(out.replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'));
assert((process.env.TEMP||'').replaceAll('\\','/').toLowerCase().startsWith('d:/buildcache/'),'Set TEMP/TMP on D');
fs.mkdirSync(out,{recursive:false});
fs.mkdirSync(path.join(out,'driver-source'));
fs.copyFileSync(__filename,path.join(out,'driver-source','test-semantic-browser.cjs'));
const base=option('--base','http://127.0.0.1:8792'),stream=option('--stream','http://127.0.0.1:8793');
const source=path.resolve(option('--source',path.resolve(__dirname,'../examples/semantic-web')));
const {chromium}=require(option('--playwright','../tests/playwright/node_modules/playwright'));
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:option('--browser','C:/Program Files/Google/Chrome/Application/chrome.exe'),downloadsPath:path.join(out,'downloads')});
 const context=await browser.newContext(),host=await context.newPage(),results=[];
 host.__GLORY_ARTIFACT_DIR=out;
 host.__GLORY_ARTIFACT_HASH=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
 host.setDefaultTimeout(60000);
 try{
  const only=option('--only','');
  const files=fs.readdirSync(source).filter(name=>(name==='acceptance.js'||name.endsWith('-acceptance.js'))&&(!only||name===only)).sort();
  for(const file of files){
   const text=fs.readFileSync(path.join(source,file),'utf8').replaceAll('http://127.0.0.1:8788',base).replaceAll('http://127.0.0.1:8789',stream);
   fs.writeFileSync(path.join(out,'driver-source',file),text);
   const test=vm.runInThisContext('('+text+')',{filename:file});
   let entry;
   try{const result=await test(host);entry={file,passed:true,result};console.log('Passed '+file);}
   catch(error){entry={file,passed:false,error:error.stack};console.error('Failed '+file+': '+error.message);}
   results.push(entry);
   fs.writeFileSync(path.join(out,file+'.json'),JSON.stringify(entry,null,2)+'\n');
  }
  fs.writeFileSync(path.join(out,'evidence.json'),JSON.stringify({browser:browser.version(),base,stream,results},null,2)+'\n');
  assert(results.every(result=>result.passed),'Some browser suites failed; see evidence.json');
 }finally{await context.close();await browser.close();}
})().catch(error=>{fs.writeFileSync(path.join(out,'failure.txt'),error.stack);console.error(error);process.exitCode=1;});
