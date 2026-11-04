document.addEventListener("DOMContentLoaded",()=>{
  const wait=async predicate=>{for(let n=0;n<800;n++){if(predicate())return;await new Promise(r=>setTimeout(r,10));}throw Error("acceptance wait timeout");};
  (async()=>{
    await wait(()=>document.querySelector(".increment"));
    const input=document.querySelector('input[aria-label="Message"]');
    document.querySelector(".increment").click();
    await wait(()=>document.querySelector(".count").textContent==="Count:1");
    input.focus(); input.value="";
    for(const character of "rapid native input 0123456789") {
      input.value+=character;
      input.dispatchEvent(new InputEvent("input",{bubbles:true,inputType:"insertText",data:character}));
    }
    document.querySelector(".capture").click();
    await wait(()=>document.querySelector(".captured").textContent==="rapid native input 0123456789");
    await wait(()=>document.querySelector(".echo").textContent==="rapid native input 0123456789");
    input.focus();input.value="中文😀";input.setSelectionRange(2,4);
    input.dispatchEvent(new InputEvent("input",{bubbles:true,inputType:"insertText",data:"中文😀"}));
    await wait(()=>document.querySelector(".echo").textContent==="中文😀");
    if(input!==document.querySelector('input[aria-label="Message"]') || input.selectionStart!==2 || input.selectionEnd!==4) throw Error("input echo replaced identity/selection");
    input.dispatchEvent(new KeyboardEvent("keydown",{key:"Enter",bubbles:true,cancelable:true}));
    document.querySelector("input[type=checkbox]").click();
    document.querySelector(".toggle").click();
    await wait(()=>!document.querySelector(".conditional"));
    document.querySelector(".toggle").click();
    await wait(()=>document.querySelector(".conditional"));
    document.querySelector(".async-start").click();
    await wait(()=>document.querySelector(".async-value").textContent==="awake");
    const moving=document.querySelector(".moving");
    moving.focus();moving.value="draft😀";moving.setSelectionRange(5,7);
    document.querySelector(".move").click();
    await wait(()=>!!(document.querySelector(".test-root").compareDocumentPosition(moving)&Node.DOCUMENT_POSITION_FOLLOWING));
    if(document.activeElement!==moving || moving.selectionStart!==5 || moving.selectionEnd!==7) throw Error("Group move lost focus/selection");
    document.querySelector(".park").click();await wait(()=>!moving.isConnected);
    document.querySelector(".resume").click();await wait(()=>moving.isConnected);
    if(document.querySelector(".moving")!==moving || moving.value!=="draft😀") throw Error("Group resume replaced input identity/value");
    input.focus();
    document.querySelector(".done").click();
  })().catch(error=>{throw error;});
},{once:true});
