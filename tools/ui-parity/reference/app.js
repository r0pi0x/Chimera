(function(){
  'use strict';
  var mq = window.matchMedia ? matchMedia('(prefers-reduced-motion: reduce)') : {matches:false};
  var RM = mq.matches;
  var TAN = 0.2126;
  var DATA = JSON.parse(document.getElementById('lbdata').textContent);
  var root = document.documentElement;
  var scenes = [];
  function clamp(x,a,b){return x<a?a:x>b?b:x}
  function c01(x){return x<0?0:x>1?1:x}
  function mod(a,n){return ((a%n)+n)%n}
  function lerp(a,b,t){return a+(b-a)*t}
  function eOut(p){p=c01(p);return p===1?1:1-Math.pow(2,-10*p)}
  function eIn(p){p=c01(p);return p===0?0:Math.pow(2,10*p-10)}
  function eio(p){p=c01(p);return p<.5?4*p*p*p:1-Math.pow(-2*p+2,3)/2}
  function eio5(p){p=c01(p);return p<.5?16*p*p*p*p*p:1-Math.pow(-2*p+2,5)/2}
  function back(p,s){p=c01(p);var q=p-1;return 1+(s+1)*q*q*q+s*q*q}
  function hold(p){p=c01(p);var q=2*p-1;return .5+.5*(q<0?-1:1)*Math.pow(Math.abs(q),3)}
  function pad2(n){return (n<10?'0':'')+n}
  function DPR(){return window.devicePixelRatio||1}
  function $(el,sel){return el.querySelector(sel)}
  function $$(el,sel){return [].slice.call(el.querySelectorAll(sel))}
  function setS(el,prop,v){if(el['_'+prop]!==v){el['_'+prop]=v;el.style[prop]=v;}}
  function setT(el,v){if(el._t!==v){el._t=v;el.textContent=v;}}
  var CLS=JSON.parse(document.getElementById('lbdata').textContent).cls;

  /* ------------------------------------------------ icons as Path2D */
  var ICON={};
  Object.keys(DATA.icons).forEach(function(k){
    var ic=DATA.icons[k], d='';
    ic.polys.forEach(function(p){d+='M'+p.map(function(q){return q[0]+' '+q[1]}).join(' L')+'Z'});
    ICON[k]={w:ic.w,polys:ic.polys,path:new Path2D(d)};
  });
  function giHTML(k){var w=DATA.icons[k].w;return '<svg class="gi" style="--w:'+w+'" viewBox="0 0 '+w+' 16" aria-hidden="true"><use href="#g-'+k+'"/></svg>'}

  /* ------------------------------------------------ palette */
  var PAL={}, PALKEY='';
  function rgbTok(cs,n){return cs.getPropertyValue(n).trim().split(/[\s,]+/).map(Number)}
  function readPal(){var cs=getComputedStyle(root);PAL.fg=rgbTok(cs,'--fg-rgb');PAL.acc=rgbTok(cs,'--acc-rgb');PAL.ink=rgbTok(cs,'--ink-rgb');PAL.threat=rgbTok(cs,'--threat-rgb');PAL.accTx=cs.getPropertyValue('--acc-tx').trim();PALKEY=PAL.acc.join(',')+PAL.fg.join(',');}
  function rgba(c,a){return 'rgba('+c[0]+','+c[1]+','+c[2]+','+a+')'}
  function hex(c){return '#'+c.map(function(v){return (v<16?'0':'')+v.toString(16)}).join('').toUpperCase()}
  var PALS=['cobalt','deep','orange'];
  function fillTokens(){
    var cs=getComputedStyle(root);
    $$(document,'code[data-tok]').forEach(function(c){var n=c.getAttribute('data-tok');var v=cs.getPropertyValue(n).trim();c.textContent=/rgb$/.test(n)?hex(v.split(/[\s,]+/).map(Number)):v.toUpperCase();});
  }
  function setPal(k,save){
    if(PALS.indexOf(k)<0)k='cobalt';
    root.setAttribute('data-pal',k);
    $$(document,'.pb,.pc').forEach(function(b){var on=b.getAttribute('data-pal')===k;b.setAttribute('aria-checked',on?'true':'false');b.tabIndex=on?0:-1;});
    if(save){try{localStorage.setItem('ugg-lb5-pal',k)}catch(e){}}
    readPal();fillTokens();
    scenes.forEach(function(s){if(s.onPal)s.onPal();});
    relayoutBars();pxProof();
    if(RM)stills();
  }
  function radios(group,items,pick){
    items.forEach(function(b,i){
      b.addEventListener('click',function(){pick(b,i)});
      b.addEventListener('keydown',function(ev){
        var d=(ev.key==='ArrowRight'||ev.key==='ArrowDown')?1:(ev.key==='ArrowLeft'||ev.key==='ArrowUp')?-1:0;
        if(!d)return; ev.preventDefault(); var j=mod(i+d,items.length); items[j].focus(); pick(items[j],j);
      });
    });
  }
  $$(document,'.palbar [role=radiogroup], .pals').forEach(function(g){
    var it=$$(g,'.pb,.pc'); radios(g,it,function(b){setPal(b.getAttribute('data-pal'),true)});
  });

  /* ------------------------------------------------ scene registry */
  function add(sel,init){
    var el=document.querySelector(sel); if(!el) return null;
    var s={el:el,t:0,vis:false,update:function(){},still:0};
    init(s); el._s=s; scenes.push(s); return s;
  }

  /* ------------------------------------------------ the logo pass (standard transition) */
  function Pass(el){
    var host=$(el,'.sting'); host.innerHTML='';
    var spec=[{w:1.1,c:'var(--acc)',off:-2.6},{w:140,c:'var(--ink)',off:0,mark:1},{w:10,c:'var(--acc)',off:141},{w:5,c:'var(--fg)',off:152}];
    var bands=spec.map(function(sp){
      var i=document.createElement('i');i.style.width=sp.w+'cqw';i.style.background=sp.c;
      if(sp.mark)i.innerHTML='<svg class="m" style="left:81.5cqw" viewBox="0 0 1248 896" aria-hidden="true"><use href="#ugg-mark"/></svg>';
      host.appendChild(i);return {el:i,off:sp.off,m:$(i,'.m')};
    });
    var lastP=null;
    return {set:function(p){
      if(!(p>=0&&p<=1))p=-1;
      if(p===lastP)return; lastP=p;
      var x=p<0?-420:-175+287*hold(p);
      bands.forEach(function(b){b.el.style.transform='translateX('+(x+b.off).toFixed(2)+'cqw) skewX(-12deg)'});
      /* the mark wipes in as the slab slows, holds, and wipes out before it leaves */
      var mi=eOut((p-.33)/.16), mo=eIn((p-.70)/.12), m=bands[1].m;
      m.style.clipPath='inset(0 '+((1-mi)*100).toFixed(1)+'% 0 '+(mo*100).toFixed(1)+'%)';
    }};
  }

  /* ------------------------------------------------ pixel-snapped tick bar */
  function tickLayout(boxW,n){var pitch=Math.max(2,Math.floor(boxW/n));var tick=Math.max(1,Math.round(pitch*.36));return {pitch:pitch,tick:tick,gap:pitch-tick,w:n*pitch-(pitch-tick),h:Math.max(3,Math.round(pitch*1.5))};}
  function TickBar(cv){this.cv=cv;this.n=+cv.getAttribute('data-n')||48;this.ctx=cv.getContext('2d');this.lit=0;this.hot=1e9;this.L=null;this.key='';}
  TickBar.prototype.layout=function(){
    var wrap=this.cv.parentElement,r=wrap.getBoundingClientRect(),d=DPR();
    if(!r.width)return;
    var L=tickLayout(Math.floor(r.width*d),this.n),th=Math.min(L.h,Math.max(3,Math.floor(r.height*d)));
    this.cv.width=L.w;this.cv.height=th;this.cv.style.width=(L.w/d)+'px';this.cv.style.height=(th/d)+'px';
    var fx=mod(r.left*d,1),fy=mod(r.top*d,1);
    this.cv.style.transform='';/* Chrome and Firefox already snap a replaced element's box to device pixels */
    this.L=L;this.th=th;this.key='';this.draw(this.lit,this.hot);
  };
  TickBar.prototype.draw=function(lit,hot){
    var key=lit+'|'+hot+'|'+PALKEY; if(key===this.key)return; this.key=key;this.lit=lit;this.hot=hot;
    var c=this.ctx,L=this.L; if(!L)return;
    c.clearRect(0,0,this.cv.width,this.cv.height);
    for(var i=0;i<this.n;i++){c.fillStyle=i<lit?(i>=hot?rgba(PAL.acc,1):rgba(PAL.fg,1)):rgba(PAL.fg,.16);c.fillRect(i*L.pitch,0,L.tick,this.th);}
  };
  var tickbars=[];
  function relayoutBars(){tickbars.forEach(function(b){b.layout()});}

  /* pixel proof: the lobby bar as it would be laid out on a 1080p, 1440p and 4K screen */
  function pxProof(){
    $$(document,'canvas.pxc').forEach(function(cv){
      var res=+cv.getAttribute('data-res'),box=Math.round(res*.35),L=tickLayout(box,48),d=DPR(),Z=3;
      var off=document.createElement('canvas');off.width=L.w;off.height=L.h;var o=off.getContext('2d');
      for(var i=0;i<48;i++){o.fillStyle=i<30?rgba(PAL.fg,1):rgba(PAL.fg,.16);if(i>=26&&i<30)o.fillStyle=rgba(PAL.acc,1);o.fillRect(i*L.pitch,0,L.tick,L.h);}
      var avail=Math.max(120,(cv.parentElement.clientWidth-24))*d;
      var nt=Math.max(2,Math.min(7,Math.floor(avail/(Z*L.pitch)))),cw=nt*L.pitch;
      cv.width=cw*Z;cv.height=L.h*Z;cv.style.width=(cw*Z/d)+'px';cv.style.height=(L.h*Z/d)+'px';
      var c=cv.getContext('2d');c.imageSmoothingEnabled=false;c.clearRect(0,0,cv.width,cv.height);
      c.drawImage(off,0,0,cw,L.h,0,0,cw*Z,L.h*Z);
      c.fillStyle=rgba(PAL.ink,.28);for(var x=Z;x<cw*Z;x+=Z)c.fillRect(x,0,1,L.h*Z);
      var k=$(cv.parentElement,'.pxk');if(k)k.textContent='Tick '+L.tick+' px • gap '+L.gap+' px • '+L.h+' px tall • bar '+L.w+' of '+box+' px';
    });
  }

  /* ------------------------------------------------ long gun names */
  var QW=/^(DESERT|WOODLAND|ARCTIC|URBAN|JUNGLE|DIGITAL|CAMO|GOLD|GOLDEN|CHROME|PLATINUM|CARBON|NEON|SKIN|EDITION|CHAMPION|LIMITED|SPECIAL|ELITE|ANNIVERSARY|TOURNAMENT|GRAND|TOPGUN|PRESTIGE|INFECTED\w*|\d{1,3})$/;
  var CW=/^(SUBMACHINE|MACHINE|GUN|ASSAULT|RIFLE|PISTOL|SHOTGUN|LAUNCHER)$/;
  var mctx=document.createElement('canvas').getContext('2d');
  var FS=.94, SLOT=12.4, L1MAX=20, MINSC=.86;
  function wid(txt,ls){mctx.font='700 100px "UGG Display"';return (mctx.measureText(txt).width/100+ls*txt.length)*FS;}
  /* line 1: the short name (game data or a per-game override), else derived; fitted; words that do not fit are dropped.
     line 2: always class and source game. */
  function fitName(raw,src,key,short){
    var base,drop=[],from;
    if(short){base=short.toUpperCase().split(/\s+/);from='short';}
    else{
      from='derived';
      var us=raw.indexOf('_');
      if(us>0){base=raw.slice(0,us).toUpperCase().split(/\s+/);drop=raw.slice(us+1).replace(/_/g,' ').toUpperCase().split(/\s+/);}
      else{var tk=raw.toUpperCase().split(/\s+/),i=1;while(i<tk.length&&!QW.test(tk[i]))i++;base=tk.slice(0,i);drop=tk.slice(i);}
      while(base.length>1&&CW.test(base[base.length-1]))drop.unshift(base.pop());
    }
    var l1w=[],over=[];base.forEach(function(w){if(!over.length&&(l1w.concat(w).join(' ').length<=L1MAX||!l1w.length))l1w.push(w);else over.push(w);});
    var l1=l1w.join(' '),ls=.04,sc=1,cut=false,tier=[];
    if(wid(l1,.04)>SLOT){ls=0;if(wid(l1,0)>SLOT){sc=SLOT/wid(l1,0);if(sc<MINSC){sc=MINSC;cut=true;while(l1.length>2&&wid(l1+'...',0)*MINSC>SLOT)l1=l1.slice(0,-1).replace(/\s+$/,'');l1+='...';}}}
    var l2=CLS[key].toUpperCase()+' • '+(src||'').toUpperCase();
    tier.push(from==='short'?'Short name used':'Short name derived');
    var nd=drop.length+over.length;
    if(nd)tier.push(nd+' word'+(nd>1?'s':'')+' dropped');
    if(cut)tier.push('line 1 cut');else if(sc<1)tier.push('condensed to '+Math.round(sc*100)+'%');else if(ls===0)tier.push('tracking tightened to 0');
    return {l1:l1,l2:l2,ls:ls,sc:sc,cut:cut,base:l1w.join(' '),drop:drop.concat(over),tier:tier};
  }
  function applyName(nmw,f){
    var a=$(nmw,'.l1'),b=$(nmw,'.l2');
    a.innerHTML=f.cut?f.l1.replace(/\.\.\.$/,'')+'<span class="ell">...</span>':f.l1;
    a.style.letterSpacing=f.ls+'em';a.style.transform=f.sc<1?'scaleX('+f.sc.toFixed(3)+')':'';
    b.textContent=f.l2;
  }
  function esc(s){return s.replace(/[&<>"]/g,function(c){return {'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]})}
  function namesPanel(){
    $$(document,'.f-names .nrow').forEach(function(r){
      var raw=r.getAttribute('data-raw'),k=r.getAttribute('data-k'),src=r.getAttribute('data-src'),sh=r.getAttribute('data-short')||'',f=fitName(raw,src,k,sh);
      applyName($(r,'.nmw'),f);
      /* mark the raw string: words not displayed are struck; a short name replaces the raw name entirely */
      var out='';
      if(sh)out='<s>'+esc(raw)+'</s>';
      else{var keep=f.base.split(' '),seen=0;
        raw.split(/([_\s]+)/).forEach(function(p){
          if(/^[_\s]+$/.test(p)||!p){out+=esc(p);return;}
          if(seen<keep.length&&p.toUpperCase()===keep[seen]){seen++;out+=esc(p);}else out+='<s>'+esc(p)+'</s>';
        });}
      $(r,'.raw').innerHTML=out;
      $(r,'.how').innerHTML='<b>'+f.tier[0]+'</b>'+f.tier.slice(1).join(' • ');
    });
  }

  /* ================================================= 01 MENU: three backdrops */
  var LADDER_API=null,SHOW_API=null,SHOWMODE='assets';
  var ICONMODE='fallback';
  /* games whose assets the mock pretends were downloaded; Combat Arms stays on the fallback set */
  var MOCKDL={'CS:S':1,'HL2:DM':1};
  var GAB={'CS:S':'CS:S','HL2:DM':'HL2:DM','Combat Arms':'CA'};
  /* a clearly-labelled placeholder for a downloaded kill icon: no real art, just a box per game style */
  function mockIcon(c,game,x,y,w,h,a,d){
    c.save();
    c.beginPath();c.rect(x,y,w,h);c.clip();
    c.fillStyle=rgba(PAL.fg,.07*a);c.fillRect(x,y,w,h);
    c.strokeStyle=rgba(PAL.fg,.5*a);c.lineWidth=Math.max(1,d*.9);c.beginPath();
    if(game==='CS:S'){var st=Math.max(4,h*.22);for(var k=-h;k<w+h;k+=st){c.moveTo(x+k+h*TAN,y);c.lineTo(x+k,y+h);}}
    else{var sp=Math.max(4,h*.2);for(var yy=y+sp/2;yy<y+h;yy+=sp)for(var xx=x+sp/2;xx<x+w;xx+=sp){c.moveTo(xx,yy);c.lineTo(xx+Math.max(1,d),yy);}}
    c.stroke();c.restore();
    c.save();c.setLineDash([4*d,3*d]);c.strokeStyle=rgba(PAL.fg,.8*a);c.lineWidth=Math.max(1,d);c.strokeRect(Math.round(x)+.5,Math.round(y)+.5,Math.round(w)-1,Math.round(h)-1);c.restore();
    if(h>14*d){var fs=Math.max(8*d,Math.min(h*.26,11*d));c.font=fs+'px "UGG Mono"';c.textAlign='center';
      var lb=GAB[game]+' ICON • MOCK',tw=c.measureText(lb).width+fs*.8;
      if(tw>w)lb='MOCK',tw=c.measureText(lb).width+fs*.8;c.fillStyle=rgba(PAL.ink,.9*a);c.fillRect(x+w/2-tw/2,y+h/2-fs*.75,tw,fs*1.5);
      c.fillStyle=rgba(PAL.fg,a);c.fillText(lb,x+w/2,y+h/2+fs*.36);}
  }
  /* A. Ladder, straight on: fourteen gun plates between two rails that lean 12°, climbed bottom to top one plate per
     kill, the knife plate crowned at the top. The brackets glide from plate to plate and lock with a zoom-in; the camera
     follows a beat later. A 14-tick meter at the right keeps the whole climb in view. */
  function Ladder(){
    /* one beat, as keyframes [time s, value in plates, easing into the key]: a gentle hoist that rises 4% past the next
       rung, an eased settle onto it (no hard stop, no clunk), then a long rest. The camera runs the same shape a little
       behind and lower; the knife rises 5%. The landing's weight is left to a soft clunk in the sound kit. */
    var LAND=.95,BEAT=1.6,N=14;
    var KB=[[0,0],[.55,1.04,'io'],[LAND,1,'io']];
    var KC=[[0,0],[.65,1.02,'io'],[LAND,1,'io']];
    var KK=[[0,0],[.58,1.05,'io'],[LAND,1,'io']];
    var EZ={io:function(p){return .5-.5*Math.cos(Math.PI*p)},o3:function(p){return 1-Math.pow(1-p,3)},i2:function(p){return p*p},o2:function(p){return 1-(1-p)*(1-p)}};
    function kf(K,tau){if(tau<=K[0][0])return K[0][1];for(var j=1;j<K.length;j++)if(tau<=K[j][0]){var a=K[j-1],b=K[j];return lerp(a[1],b[1],EZ[b[2]]((tau-a[0])/(b[0]-a[0])));}return K[K.length-1][1];}
    var END=(N-1)*BEAT+LAND,P=END+2.2+.55;
    var CROWN=new Path2D('M0 14L2 2L7 8L11 0L15 8L20 2L22 14Z');
    function geo(w,h){var S=h*.17;return {S:S,PH:S*.6,PW:w*.2,cx:w*.585,ay:h*.6};}
    function railX(G,side,y){return G.cx+side*G.PW/2+(G.ay-y)*TAN;}
    function plate(G,i,c){var y=G.ay-(i-c)*G.S,top=y-G.PH/2,bot=y+G.PH/2;
      return {y:y,top:top,bot:bot,cx:G.cx+(G.ay-y)*TAN,q:[[railX(G,-1,top),top],[railX(G,1,top),top],[railX(G,1,bot),bot],[railX(G,-1,bot),bot]]};}
    function iconBox(G,i,c){var p=plate(G,i,c),g=DATA.ladder[i],ic=ICON[g[2]],ih=G.PH*.5,us=Math.min(ih/16,G.PW*.6/ic.w),iw=ic.w*us;
      var dl=ICONMODE==='dl'&&MOCKDL[g[1]];if(dl){iw=Math.min(G.PW*.6,16*us*(g[1]==='CS:S'?2.9:2.3));}
      return {cx:p.cx,cy:p.y,hw:iw/2,hh:8*us,us:us,dl:dl};}
    /* beat k (k >= 1) carries plate k-1 to plate k on the keyframes above; landing at LAND s into the beat */
    function state(tt){
      var k=Math.min(N-1,Math.floor(tt/BEAT)),tau=tt-k*BEAT;
      if(k===0)return {b:0,c:0,k:0,tau:9,land:9,fin:false,arr:-1};
      var fin=k===N-1;
      return {b:k-1+kf(fin?KK:KB,tau),c:k-1+kf(KC,tau),k:k,tau:tau,land:tau-LAND,fin:fin,arr:fin?tau-LAND:-1};
    }
    /* landing feedback: a camera kick down that rises in 20 ms and settles in 70 ms; a 70 ms edge flash */
    /* no camera kick, no edge flash: line widths and brightness stay constant; the meter tick lights on landing */
    function zoom(st){var Z=st.fin?.08:.06;return Z*Math.sin(Math.PI*c01(st.tau/LAND));}
    function pgram(c,q){c.beginPath();c.moveTo(q[0][0],q[0][1]);for(var j=1;j<4;j++)c.lineTo(q[j][0],q[j][1]);c.closePath();}
    var api={still:END+1.2,period:function(){return P},geo:function(w,h,c){var G=geo(w,h);return {plate:plate(G,N-1,c==null?N-1:c),railL:function(y){return railX(G,-1,y)},railR:function(y){return railX(G,1,y)}};},
      draw:function(c,w,h,t,d){
      var tt=RM?api.still:mod(t,P),G=geo(w,h),st=state(tt),cc=st.c,b=st.b,hair=Math.max(1,Math.round(d));
      var A=RM?1:c01(tt/.45)*(1-eio((tt-(P-.55))/.55));
      if(A<=0)return;
      c.save();c.globalAlpha=A;
      /* rails */
      c.lineWidth=Math.max(1,1.2*d);
      var rTop=Math.max(-h*.05,plate(G,N-1,cc).top-G.PH*.9);
      [-1,1].forEach(function(sd){var g=c.createLinearGradient(0,0,0,h);g.addColorStop(0,rgba(PAL.fg,0));g.addColorStop(.18,rgba(PAL.fg,.26));g.addColorStop(.85,rgba(PAL.fg,.26));g.addColorStop(1,rgba(PAL.fg,0));
        c.strokeStyle=g;c.beginPath();c.moveTo(railX(G,sd,rTop),rTop);c.lineTo(railX(G,sd,h*1.05),h*1.05);c.stroke();});
      var fin=null;
      for(var i=Math.max(0,Math.floor(cc-4));i<=Math.min(N-1,Math.ceil(cc+4));i++){
        var p=plate(G,i,cc);if(p.bot<-h*.05||p.top>h*1.05)continue;
        var vf=c01((p.y+G.PH*.2)/(h*.2))*c01((h*1.06-p.y)/(h*.22)),hl=c01(1-Math.abs(i-b)*1.4),last=i===N-1,done=i<b-.5;
        if(vf<=0)continue;
        c.save();c.globalAlpha=A*vf;
        pgram(c,p.q);
        c.fillStyle=last?rgba(PAL.acc,.12+.1*hl):rgba(PAL.fg,done?.05:(.03+.05*hl));c.fill();
        c.strokeStyle=last?rgba(PAL.acc,.85):rgba(PAL.fg,.16+.4*hl);c.lineWidth=hair;c.stroke();
        if(last){fin=p;
          /* finish chequer at both ends of the plate, cells on the 12° slant */
          var rows=4,cs=G.PH/rows;
          [-1,1].forEach(function(sd){for(var r=0;r<rows;r++)for(var col=0;col<2;col++){if((r+col)%2)continue;
            var y0=p.top+r*cs,y1=y0+cs,o0=sd<0?col*cs:-(col+1)*cs,o1=o0+cs;
            c.beginPath();c.moveTo(railX(G,sd,y0)+o0,y0);c.lineTo(railX(G,sd,y0)+o1,y0);c.lineTo(railX(G,sd,y1)+o1,y1);c.lineTo(railX(G,sd,y1)+o0,y1);c.closePath();
            c.fillStyle=rgba(PAL.acc,.7);c.fill();}});
          /* crown over the plate centre */
          var chh=G.PH*.42,csx=chh/14,cby=p.top-G.PH*.2,cbx=p.cx+(p.y-cby)*TAN;c.save();c.translate(cbx-11*csx,cby-14*csx);c.scale(csx,csx);c.transform(1,0,-TAN,1,TAN*14,0);
          c.fillStyle=rgba(PAL.acc,.95);c.fill(CROWN);c.restore();
        }
        /* icon */
        var g=DATA.ladder[i],ib=iconBox(G,i,cc),ia=last?(.55+.45*hl):(done?.32:.38)+.62*hl;
        if(ib.dl)mockIcon(c,g[1],ib.cx-ib.hw,ib.cy-ib.hh,2*ib.hw,2*ib.hh,ia,d);
        else{c.save();c.translate(ib.cx-ib.hw,ib.cy-ib.hh);c.scale(ib.us,ib.us);c.fillStyle=last?rgba(PAL.fg,ia):rgba(PAL.fg,ia);c.fill(ICON[g[2]].path,'evenodd');c.restore();}
        /* rung number left of the rail */
        c.font=Math.round(h*.019)+'px "UGG Mono"';c.textAlign='right';
        c.fillStyle=last?rgba(PAL.acc,.95):rgba(PAL.fg,.32+.6*hl);c.fillText(pad2(i+1),railX(G,-1,p.y)-14*d,p.y+h*.0065);
        /* final plate tag, always */
        if(last&&hl<.5){c.textAlign='left';c.fillStyle=rgba(PAL.acc,.95);c.font=Math.round(h*.017)+'px "UGG Mono"';c.fillText('FINAL • KNIFE',railX(G,1,p.y)+16*d,p.y+h*.006);}
        c.restore();
      }
      /* glint across the knife plate on arrival */
      if(fin&&st.arr>=0){var gq=(st.arr-.18)/.75;if(gq>0&&gq<1){c.save();pgram(c,fin.q);c.clip();
        var gx=lerp(railX(G,-1,fin.y)-G.PH,railX(G,1,fin.y)+G.PH,eio(gq)),gr=c.createLinearGradient(gx-G.PH*.9,0,gx+G.PH*.9,0),k=Math.sin(Math.PI*gq);
        gr.addColorStop(0,rgba(PAL.fg,0));gr.addColorStop(.5,rgba(PAL.fg,.35*k));gr.addColorStop(1,rgba(PAL.fg,0));
        c.transform(1,0,-TAN,1,TAN*fin.y,0);c.fillStyle=gr;c.fillRect(gx-G.PH*2,fin.top-4,G.PH*4,G.PH+8);c.restore();}}
      /* brackets: glide between plates, zoom in to lock; continuous across beats */
      var i0=clamp(Math.floor(b),0,N-2),i1=i0+1,fr=b-i0,B0=iconBox(G,i0,cc),B1=iconBox(G,i1,cc);
      var bx=lerp(B0.cx,B1.cx,fr),by=lerp(B0.cy,B1.cy,fr),pad=G.PH*.2,bw=lerp(B0.hw,B1.hw,fr)+pad,bh=lerp(B0.hh,B1.hh,fr)+pad;
      var zs=zoom(st),cl=G.PH*.2;
      function brackets(s,a,lw){c.strokeStyle=rgba(PAL.acc,a);c.lineWidth=lw;c.beginPath();
        [[-1,-1],[1,-1],[1,1],[-1,1]].forEach(function(q){var px=bx+q[0]*bw*s,py=by+q[1]*bh*s;c.moveTo(px-q[0]*cl,py);c.lineTo(px,py);c.lineTo(px,py-q[1]*cl);});c.stroke();}
      brackets(1+zs,1,Math.max(1.5,1.6*d));api.last={x0:bx-bw*(1+zs),x1:bx+bw*(1+zs),y0:by-bh*(1+zs),y1:by+bh*(1+zs)};
      if(st.arr>=0){var aq=c01(st.arr/.7);if(aq<1)brackets(1+.55*eOut(aq),1-aq,Math.max(1,1.2*d));
        var aq2=c01((st.arr-.25)/.8);if(aq2>0&&aq2<1)brackets(1+.55*eOut(aq2),.6*(1-aq2),Math.max(1,d));}
      /* label for the current plate */
      var kk=clamp(Math.round(b),0,N-1),pk=plate(G,kk,cc),la=c01(1-Math.abs(b-kk)*2.4),g2=DATA.ladder[kk],lx=railX(G,1,pk.top)+18*d,ly=pk.y-G.PH*.28;
      if(la>0){c.globalAlpha=A*la;c.textAlign='left';var lastK=kk===N-1;
        c.fillStyle=lastK?rgba(PAL.acc,1):rgba(PAL.fg,1);c.font='700 '+Math.round(h*.064)+'px "UGG Display"';c.fillText(pad2(kk+1),lx,ly+h*.012);
        c.font=Math.round(h*.019)+'px "UGG Mono"';c.fillStyle=rgba(PAL.fg,.85);c.fillText(lastK?'KNIFE':g2[0].toUpperCase(),lx,ly+h*.046);
        c.fillStyle=lastK?rgba(PAL.acc,.95):rgba(PAL.fg,.45);c.fillText(lastK?'FINAL • KNIFE WINS':(g2[1].toUpperCase()+' • '+CLS[g2[2]].toUpperCase()),lx,ly+h*.074);
        if(ICONMODE==='dl'){var got=MOCKDL[g2[1]];c.fillStyle=got?rgba(PAL.acc,1):rgba(PAL.fg,.45);c.fillRect(lx,ly+h*.092,h*.008,h*.014);
          c.fillStyle=rgba(PAL.fg,got?.8:.45);c.fillText(got?'ICON • DOWNLOADED (MOCK)':'ICON • FALLBACK, NONE DOWNLOADED',lx+h*.016,ly+h*.104);}
        c.globalAlpha=A;}
      /* the whole climb: 14 ticks on the slant, knife crowned at the top */
      var mx=w*.905,my0=h*.76,my1=h*.24,tw=h*.03,th=Math.max(2*d,Math.round(h*.011));
      for(var r=0;r<N;r++){var yy=lerp(my0,my1,r/(N-1)),xx=mx+(h*.5-yy)*TAN*.35,lr=st.k===0||st.land>=0?st.k:st.k-1,on=r<=lr,cur=r===lr;
        c.beginPath();c.moveTo(xx+th*TAN,yy-th/2);c.lineTo(xx+tw+th*TAN,yy-th/2);c.lineTo(xx+tw,yy+th/2);c.lineTo(xx,yy+th/2);c.closePath();
        if(r===N-1){c.fillStyle=on?rgba(PAL.acc,1):rgba(PAL.acc,.25);c.fill();c.strokeStyle=rgba(PAL.acc,.9);c.lineWidth=hair;c.stroke();
          var s2=h*.012/14;c.save();c.translate(xx+tw/2-11*s2,yy-th/2-h*.018);c.scale(s2,s2);c.fillStyle=rgba(PAL.acc,.95);c.fill(CROWN);c.restore();}
        else{c.fillStyle=cur?rgba(PAL.acc,1):on?rgba(PAL.fg,.8):rgba(PAL.fg,.14);c.fill();}}
      c.font=Math.round(h*.015)+'px "UGG Mono"';c.textAlign='left';c.fillStyle=rgba(PAL.fg,.45);
      c.fillText('RUNG '+pad2(Math.round(b)+1)+' / 14',mx,my0+h*.045);
      c.restore();
    }};
    return api;
  }

  /* B. Assembly: each gun is drafted on a 16-unit grid, traced by a 12° scan, hatched, filled, dimensioned,
     then filed into a shelf of every game's guns. Nothing flies; one thing happens at a time. */
  function Assembly(){
    var SEQ=[['revolver','HL2:DM'],['smg','Combat Arms'],['shotgun','HL2:DM'],['rifle','Combat Arms'],['lmg','CS:S'],['sniper','CS:S'],['crossbow','HL2:DM'],['knife','CS:S']];
    var SL=3.4,P=SEQ.length*SL;
    var RG=SEQ.map(function(q){var rs=[];DATA.ladder.forEach(function(l,i){if(l[2]===q[0])rs.push(i+1)});return rs.length>1?'RUNGS '+pad2(rs[0])+'–'+pad2(rs[rs.length-1]):rs.length?'RUNG '+pad2(rs[0]):'';});
    /* per gun: edges with their scan coordinate, the y-levels for guides, scan range */
    var G=SEQ.map(function(q){
      var ic=DATA.icons[q[0]],E=[],ys={},qmin=1e9,qmax=-1e9;
      ic.polys.forEach(function(p){for(var i=0;i<p.length;i++){var a=p[i],b=p[(i+1)%p.length];E.push([a[0],a[1],b[0],b[1]]);if(a[1]===b[1]&&Math.abs(b[0]-a[0])>=4)ys[a[1]]=1;
        [a,b].forEach(function(v){var s=v[0]-(8-v[1])*TAN;if(s<qmin)qmin=s;if(s>qmax)qmax=s;});}});
      return {w:ic.w,E:E,ys:Object.keys(ys).map(Number).sort(function(a,b){return a-b}),q0:qmin-.6,q1:qmax+.6,path:ICON[q[0]].path};
    });
    function sq(x,y){return x-(8-y)*TAN}
    return {still:12.6,draw:function(c,w,h,t,d){
      var tt=mod(t,P),j=Math.floor(tt/SL),tau=tt-j*SL,g=G[j];
      var U=Math.min(w*.36/54,h*.40/16),cx=w*.665,cy=h*.40,ox=cx-g.w*U/2,oy=cy-8*U;
      function X(u){return ox+u*U}function Y(v){return oy+v*U}
      var hair=Math.max(1,Math.round(d)),hp=hair%2?.5:0;
      /* exit: a 12° wipe that erases left to right */
      var ex=c01((tau-2.95)/.4),fe=lerp(g.q0,g.q1,eio(ex));
      c.save();
      if(ex>0){c.beginPath();var sk=(16+12)*U*TAN;c.moveTo(X(fe)+(8+6)*U*TAN,Y(-6));c.lineTo(w*2,Y(-6));c.lineTo(w*2,Y(22));c.lineTo(X(fe)-(22-8)*U*TAN,Y(22));c.closePath();c.clip();}
      /* 1. the grid: one dot per unit inside the gun's box, then its guides */
      var ga=eOut(tau/.45);
      c.fillStyle=rgba(PAL.fg,.16*ga);
      var ds=Math.max(1,Math.round(d*1.2));
      for(var gy=0;gy<=16;gy++)for(var gx=0;gx<=g.w;gx++){c.fillRect(Math.round(X(gx))-(ds>>1),Math.round(Y(gy))-(ds>>1),ds,ds);}
      c.lineWidth=hair;c.strokeStyle=rgba(PAL.fg,.075*ga);c.beginPath();
      var gl=lerp(0,w*.16,eOut(tau/.7));
      g.ys.forEach(function(v){var yy=Math.round(Y(v))+hp;c.moveTo(X(0)-gl,yy);c.lineTo(X(g.w)+gl,yy);});
      [0,g.w].forEach(function(u){var xx=Math.round(X(u))+hp;c.moveTo(xx,Y(0)-gl*.5);c.lineTo(xx,Y(16)+gl*.5);});
      c.stroke();
      /* 2. the scan traces the outline */
      var sp=c01((tau-.3)/1.15),f=lerp(g.q0,g.q1,eio(sp));
      var ol=new Path2D();
      g.E.forEach(function(e){
        var s0=sq(e[0],e[1]),s1=sq(e[2],e[3]),lo=Math.min(s0,s1);if(lo>f)return;
        var tcut=1;if(Math.max(s0,s1)>f){tcut=(f-s0)/(s1-s0);if(s1<s0)tcut=1;}
        var x0=e[0],y0=e[1],x1=e[2],y1=e[3];
        if(s1<s0&&Math.max(s0,s1)>f){var tc=(f-s1)/(s0-s1);x0=lerp(e[2],e[0],tc);y0=lerp(e[3],e[1],tc);x1=e[2];y1=e[3];}
        else if(tcut<1){x1=lerp(e[0],e[2],tcut);y1=lerp(e[1],e[3],tcut);}
        ol.moveTo(Math.round(X(x0))+hp,Math.round(Y(y0))+hp);ol.lineTo(Math.round(X(x1))+hp,Math.round(Y(y1))+hp);
      });
      /* 3. hatch at 12°, following a second, slower scan; 4. the fill settles over it */
      var hpz=c01((tau-1.0)/1.0),fh=lerp(g.q0,g.q1,eio(hpz)),fa=.86*eOut((tau-1.85)/.45);
      if(hpz>0){
        c.save();c.translate(ox,oy);c.scale(U,U);c.beginPath();c.moveTo(-3,-2);c.lineTo(fh+10*TAN,-2);c.lineTo(fh-10*TAN,18);c.lineTo(-3,18);c.closePath();c.clip();
        c.save();c.clip(g.path,'evenodd');c.setTransform(1,0,0,1,0,0);
        c.strokeStyle=rgba(PAL.fg,.34*(1-fa));c.lineWidth=hair;c.beginPath();
        var st=Math.max(3*d,U*.75);for(var k=X(-4);k<X(g.w+6);k+=st){c.moveTo(Math.round(k+16*U*TAN),Y(0));c.lineTo(Math.round(k),Y(16));}
        c.stroke();c.restore();c.restore();
      }
      if(fa>0){c.save();c.translate(ox,oy);c.scale(U,U);c.fillStyle=rgba(PAL.fg,fa);c.fill(g.path,'evenodd');c.restore();}
      c.lineWidth=Math.max(1,Math.round(1.5*d));c.lineCap='square';c.strokeStyle=rgba(PAL.fg,.95);c.stroke(ol);c.lineCap='butt';
      /* the pen: an accent hairline on the slant, at the scan front */
      if(sp>0&&sp<1){var pa=Math.sin(Math.PI*sp);c.strokeStyle=rgba(PAL.acc,pa);c.lineWidth=Math.max(1.5,1.5*d);c.beginPath();
        c.moveTo(X(f)+(8+3)*U*TAN,Y(-3));c.lineTo(X(f)-(19-8)*U*TAN,Y(19));c.stroke();}
      /* 5. dimensions */
      var da=eOut((tau-1.55)/.4);
      if(da>0){
        c.strokeStyle=rgba(PAL.fg,.42*da);c.fillStyle=rgba(PAL.fg,.55*da);c.lineWidth=hair;c.beginPath();
        var yd=Math.round(Y(16)+U*2.2)+hp,xd=Math.round(X(0)-U*2.2)+hp,tk=U*.6,xr=X(0)+(X(g.w)-X(0))*da;
        c.moveTo(X(0),yd);c.lineTo(xr,yd);c.moveTo(X(0),yd-tk);c.lineTo(X(0),yd+tk);c.moveTo(xr,yd-tk);c.lineTo(xr,yd+tk);
        var yb=Y(16)-(Y(16)-Y(0))*da;c.moveTo(xd,Y(16));c.lineTo(xd,yb);c.moveTo(xd-tk,Y(16));c.lineTo(xd+tk,Y(16));c.moveTo(xd-tk,yb);c.lineTo(xd+tk,yb);
        c.stroke();
        c.font=Math.round(h*.016)+'px "UGG Mono"';c.textAlign='center';
        var lb=g.w+' U',lw2=c.measureText(lb).width+10*d;c.clearRect((X(0)+X(g.w))/2-lw2/2,yd-h*.012,lw2,h*.024);
        c.fillStyle=rgba(PAL.fg,.6*da);c.fillText(lb,(X(0)+X(g.w))/2,yd+h*.0055);
        c.textAlign='right';c.fillText('16 U',xd-tk-4*d,(Y(0)+Y(16))/2+h*.0055);
      }
      c.restore();
      /* label: class and source game */
      var la=eOut((tau-1.9)/.35)*(1-c01((tau-2.95)/.25)),lx=X(0),ly=Y(16)+U*2.2+h*.085;
      if(la>0){
        c.save();c.beginPath();c.rect(lx-4,ly-h*.06,(w*.4)*la,h*.1);c.clip();
        c.textAlign='left';c.fillStyle=rgba(PAL.fg,1);c.font='700 '+Math.round(h*.042)+'px "UGG Display"';c.fillText(CLS[SEQ[j][0]].toUpperCase(),lx,ly);
        c.font=Math.round(h*.017)+'px "UGG Mono"';c.fillStyle=rgba(PAL.fg,.55);c.fillText(SEQ[j][1].toUpperCase()+(RG[j]?' • LADDER '+RG[j]:''),lx,ly+h*.032);
        c.restore();
        c.fillStyle=rgba(PAL.acc,la);c.fillRect(lx-12*d,ly-h*.034,3*d,h*.072);
      }
      /* header readout */
      c.textAlign='left';c.font=Math.round(h*.016)+'px "UGG Mono"';c.fillStyle=rgba(PAL.fg,.42);
      var hd=tau<.3?'DRAFT':tau<1.45?'TRACE':tau<1.9?'HATCH':tau<2.95?'FILL • DIMENSION':'FILE';
      c.fillText('BUILD '+pad2(j+1)+' / '+pad2(SEQ.length)+' • '+hd,X(0),Y(0)-U*2.4);
      /* 6. the shelf: every gun built this cycle, filed under its game */
      var n=SEQ.length,sx0=w*.49,sx1=w*.93,sw=(sx1-sx0)/n,sy=h*.70,sh=h*.03;
      c.lineWidth=hair;
      for(var k2=0;k2<n;k2++){
        var bx=sx0+k2*sw,done=k2<j||(k2===j&&tau>2.95),ic=G[k2],us=Math.min(sh/16,(sw-14*d)/ic.w),iw=ic.w*us,curk=k2===j;
        var fin=k2===j?eOut((tau-2.95)/.3):1;
        c.fillStyle=rgba(PAL.fg,.09);c.fillRect(Math.round(bx),Math.round(sy+sh+h*.012),Math.round(sw-8*d),hair);
        if(done){c.save();c.translate(bx+(sw-8*d)/2-iw/2,sy+sh-16*us);c.scale(us,us);c.fillStyle=rgba(PAL.fg,(curk?.95:.5)*fin);c.fill(ic.path,'evenodd');c.restore();}
        else{c.save();c.setLineDash([2*d,3*d]);c.strokeStyle=rgba(PAL.fg,.14);c.strokeRect(Math.round(bx+(sw-8*d)/2-iw/2)+hp,Math.round(sy+sh-16*us)+hp,Math.round(iw),Math.round(16*us));c.restore();}
        c.font=Math.round(h*.0135)+'px "UGG Mono"';c.textAlign='left';c.fillStyle=rgba(PAL.fg,curk?.75:.32);
        c.fillText(GAB[SEQ[k2][1]],Math.round(bx),Math.round(sy+sh+h*.036));
        if(curk){c.fillStyle=rgba(PAL.acc,1);c.fillRect(Math.round(bx),Math.round(sy+sh+h*.012)-hair,Math.round((sw-8*d)*c01(tau/2.95)),hair*2);}
      }
    }};
  }

  /* C. Ident: the mark signs on like a channel ident. A 12° construction grid draws in behind it and fades; one accent
     scan with a soft cobalt wake crosses on the slant, and each of the six facets lights and settles inward as the scan
     meets it; on lock a cobalt bloom rises behind the mark and an outline flash runs round it; through the hold a slow
     light breathes across the facets and a sheen with a real falloff crosses twice; the scan signs it off, facets
     easing outward as they dim. No pointer; nothing on the mark but light. */
  function Ident(){
    var built=false,P=[],SQ=[],CU=[],sq0=1e9,sq1=-1e9,VC=658,UC=624;
    var BASE=[.88,.72,.85,.9,.74,.8];
    var T0=.4,T1=2.2,LK=.9,OFF0=13.6,OFF1=15.3,SHEEN=[4.0,10.0],SH=2.8;
    function build(){
      built=true;
      $$(document,'#ugg-mark path').forEach(function(p){
        var d=p.getAttribute('d'),n=d.match(/-?\d*\.?\d+/g).map(Number),lo=1e9,hi=-1e9,su=0,sv=0,m=0;
        for(var i=0;i+1<n.length;i+=2){var q=n[i]-(VC-n[i+1])*TAN;if(q<lo)lo=q;if(q>hi)hi=q;su+=n[i];sv+=n[i+1];m++;}
        P.push(new Path2D(d));SQ.push([lo,hi]);CU.push([su/m,sv/m]);if(lo<sq0)sq0=lo;if(hi>sq1)sq1=hi;
      });
    }
    return {still:7.0,draw:function(c,w,h,t,d){
      if(!built)build();
      var tt=mod(t,16),K=h*.5/896,ox=w*.665-UC*K,oy=h*.45-448*K;
      function X(u){return ox+u*K}function Y(v){return oy+(v-210)*K}
      var yT=Y(210)-h*.035,yB=Y(1106)+h*.035,yc=Y(VC),hair=Math.max(1,Math.round(d)),hp=hair%2?.5:0;
      var pad=60,fOn=lerp(sq0-pad,sq1+pad,eio((tt-T0)/(T1-T0))),fOff=lerp(sq0-pad,sq1+pad,eio((tt-OFF0)/(OFF1-OFF0)));
      var on=tt>=T0,off=tt>=OFF0,gone=tt>=OFF1,hold=c01((tt-T1)/1.2)*(1-c01((tt-OFF0)/.6));
      var bx0=X(-48),bx1=X(1296),by0=Y(210)-h*.02,by1=Y(1106)+h*.02;
      function front(fq,left){var x0=X(fq)+(yc+h)*TAN,x1=X(fq)+(yc-2*h)*TAN;c.beginPath();
        if(left){c.moveTo(-w,-h);c.lineTo(x0,-h);c.lineTo(x1,2*h);c.lineTo(-w,2*h);}else{c.moveTo(x0,-h);c.lineTo(3*w,-h);c.lineTo(3*w,2*h);c.lineTo(x1,2*h);}c.closePath();}
      /* 1. construction grid on the slant, drawn in by the scan, gone soon after lock */
      var ga=.075*(1-eio((tt-T1-.1)/1.5));
      if(on&&ga>0){c.save();front(fOn,true);c.clip();c.beginPath();c.rect(bx0,by0,bx1-bx0,by1-by0);c.clip();
        c.strokeStyle=rgba(PAL.fg,ga);c.lineWidth=hair;c.beginPath();
        for(var u=-48;u<=1296+400;u+=112){var xa=X(u)+(yc-by0)*TAN,xb=X(u)+(yc-by1)*TAN;c.moveTo(Math.round(xa)+hp,by0);c.lineTo(Math.round(xb)+hp,by1);}
        for(var v=210;v<=1106;v+=112){var yy=Math.round(Y(v))+hp;c.moveTo(bx0,yy);c.lineTo(bx1,yy);}
        c.stroke();c.restore();}
      /* 2. bloom behind the mark: rises on lock, settles low through the hold */
      var lp=(tt-T1)/LK,bl=on&&!gone?(.13*Math.exp(-Math.max(0,lp)*2.2)*c01(lp*6)+.05*hold)*(1-c01((tt-OFF0)/1.4)):0;
      if(bl>.002){var mx=X(UC),my=Y(VC),R=h*.42;c.save();c.translate(mx,my);c.scale(1.35,1);
        var rg=c.createRadialGradient(0,0,0,0,0,R);for(var q=0;q<=8;q++){var r=q/8;rg.addColorStop(r,rgba(PAL.acc,(bl*Math.exp(-r*r*3.2)*(1-r)).toFixed(4)));}
        c.fillStyle=rg;c.fillRect(-R,-R,2*R,2*R);c.restore();}
      /* 3. facets: sign on along the scan, settle inward; breathe through the hold; ease outward on sign-off */
      if(on&&!gone){
        c.save();
        if(!off){front(fOn,true);c.clip();}
        for(var i=0;i<P.length;i++){
          var span=Math.max(1,SQ[i][1]-SQ[i][0]),lit=off?1:c01((fOn-SQ[i][0])/span),e=eOut(lit);
          var eo=off?eio(c01((fOff-SQ[i][0])/span)):0,dx=(CU[i][0]-UC)*(.06*(1-e)+.05*eo),dy=(CU[i][1]-VC)*(.06*(1-e)+.05*eo);
          var br=.05*hold*Math.sin(2*Math.PI*(tt/7)-CU[i][0]/420),a=Math.max(0,(BASE[i]+br)*e*(1-eo));
          if(a<=.003)continue;
          c.save();c.setTransform(K,0,0,K,ox+dx*K,oy-210*K+dy*K);c.fillStyle=rgba(PAL.fg,a.toFixed(3));c.fill(P[i],'evenodd');
          /* 4. sheen: a soft band with a bright core and a long falloff, light on the facet only */
          SHEEN.forEach(function(s0){var p=(tt-s0)/SH;if(p<=0||p>=1||off)return;
            var cx=lerp(-260,1500,eio(p)),k=Math.sin(Math.PI*p),g=c.createLinearGradient(cx-260,0,cx+260,0);
            [[0,0],[.2,.015],[.35,.05],[.44,.13],[.49,.3],[.5,.38],[.51,.3],[.56,.13],[.65,.05],[.8,.015],[1,0]].forEach(function(s){g.addColorStop(s[0],rgba(PAL.fg,(s[1]*k).toFixed(4)));});
            c.save();c.clip(P[i],'evenodd');c.transform(1,0,-TAN,1,TAN*VC,0);c.fillStyle=g;c.fillRect(-700,0,2800,1400);c.restore();});
          c.restore();
        }
        /* 5. lock: an outline flash with a soft glow */
        if(lp>=0&&lp<1&&!off){c.save();c.setTransform(K,0,0,K,ox,oy-210*K);c.strokeStyle=rgba(PAL.acc,(.95*(1-lp)*(1-lp)).toFixed(3));c.lineWidth=Math.max(1.2,1.5*d)/K;c.lineJoin='miter';
          c.shadowColor=rgba(PAL.acc,.8*(1-lp));c.shadowBlur=14*d*(1-lp);for(var m=0;m<P.length;m++)c.stroke(P[m]);c.restore();}
        c.restore();
      }
      /* the scan: one accent hairline with a soft cobalt wake behind it */
      var scan=tt>=T0&&tt<T1+.05?[fOn,(tt-T0)/(T1-T0)]:(off&&!gone?[fOff,(tt-OFF0)/(OFF1-OFF0)]:null);
      if(scan){var al=Math.sin(Math.PI*c01(scan[1])),fx=X(scan[0]),wk=h*.16;
        c.save();c.beginPath();c.moveTo(fx+(yc-yT)*TAN,yT);c.lineTo(fx+(yc-yB)*TAN,yB);c.lineTo(fx+(yc-yB)*TAN-wk,yB);c.lineTo(fx+(yc-yT)*TAN-wk,yT);c.closePath();
        var wg=c.createLinearGradient(fx-wk,0,fx,0);wg.addColorStop(0,rgba(PAL.acc,0));wg.addColorStop(1,rgba(PAL.acc,.16*al));c.fillStyle=wg;c.fill();c.restore();
        c.strokeStyle=rgba(PAL.acc,al);c.lineWidth=Math.max(1.5,1.6*d);c.beginPath();c.moveTo(fx+(yc-yT)*TAN,yT);c.lineTo(fx+(yc-yB)*TAN,yB);c.stroke();}
      /* safe-frame brackets and the ident readout */
      var bl2=h*.03;
      c.strokeStyle=rgba(PAL.fg,.4);c.lineWidth=hair;c.beginPath();
      [[bx0,by0,1,1],[bx1,by0,-1,1],[bx0,by1,1,-1],[bx1,by1,-1,-1]].forEach(function(q){var x=Math.round(q[0])+hp,y=Math.round(q[1])+hp;c.moveTo(x+q[2]*bl2,y);c.lineTo(x,y);c.lineTo(x,y+q[3]*bl2);});c.stroke();
      var stx=!on||gone?'STANDBY':tt<T1?'ACQUIRING':off?'SIGNING OFF':'LOCKED',locked=stx==='LOCKED';
      var fs=Math.round(h*.017),ry=by1+h*.045,sq=Math.round(h*.013);
      c.font=fs+'px "UGG Mono"';c.textAlign='left';
      if(locked){c.fillStyle=rgba(PAL.acc,1);c.fillRect(Math.round(bx0),Math.round(ry-sq),sq,sq);}
      else{c.strokeStyle=rgba(PAL.fg,.5);c.lineWidth=hair;c.strokeRect(Math.round(bx0)+hp,Math.round(ry-sq)+hp,sq-hair,sq-hair);}
      c.fillStyle=rgba(PAL.fg,locked?.7:.45);c.fillText('IDENT • '+stx,bx0+sq+h*.012,ry);
      c.textAlign='right';c.fillStyle=rgba(PAL.fg,.4);c.fillText('CH 01 • ULTIMATE GUN GAME',bx1,ry);
    }};
  }

  /* C. Showcase: the Ident's motion language applied to an asset. In game this is the in-engine 3D showcase (a real gun,
     or a character holding a cross-game gun); here a MOCK: our own fallback icon extruded into a bevelled slab.
     Materialize: 12° grid and measurement frame draw in; a wire pass traces it on the scan with the cobalt wake; the shaded
     body fills in behind the scan; bloom and outline flash on lock. Hold: a slow inspect orbit, tilt and float, the sheen,
     a label plate with spec ticks. Sign-off: back to wire behind the scan, then away; the next asset follows. */
  function Showcase(){
    /* stats from our weapon data (rate = 60 / trigger cycle); Combat Arms' AK-47 has no data file yet (its values come from
       the install at run time), so it shows clearly labelled sample values */
    var SH=[{k:'rifle',n:'AK-47',g:'COMBAT ARMS',cyc:.1,dmg:36,mag:30,src:'SAMPLE • FROM THE INSTALL IN GAME'},
            {k:'sniper',n:'AWP',g:'CS:S',cyc:1.5,dmg:115,mag:10,src:'CS_SOURCE/DATA/WEAPONS/AWP.TOML'},
            {k:'energy',n:'AR2',g:'HL2:DM',cyc:.1,dmg:11,mag:30,src:'HL2DM/DATA/WEAPONS/AR2.TOML + _GUN.TOML'},
            {k:'smg',n:'P90',g:'CS:S',cyc:.07,dmg:26,mag:50,src:'CS_SOURCE/DATA/WEAPONS/P90.TOML'}];
    SH.forEach(function(a){DATA.ladder.forEach(function(l,i){if(l[0].toUpperCase()===a.n)a.r=i+1;});});
    var P=16,DEPTH=3.2,CAM=110;
    var W0=.3,W1=2.1,F0=1.0,F1=2.9,LOCK=2.9,LB0=3.3,LB1=3.8,LBO=13.0,O0=13.4,O1=14.8,WO0=13.9,WO1=15.3;
    function ang(x){return x*Math.PI/180}
    var api={still:7.5,draw:function(c,w,h,t,d){
      var tt=RM?api.still:mod(t,P),j=RM?0:mod(Math.floor(t/P),SH.length),a=SH[j],ic=DATA.icons[a.k],IW=ic.w;
      var U=w*.26/48,fl=RM?0:.6*U*Math.sin(.5*tt),ox=w*.67,oy=h*.355+fl,hair=Math.max(1,Math.round(d)),hp=hair%2?.5:0;
      var A=ang(-27+9*Math.sin(.31*tt)),B=ang(13+3*Math.sin(.27*tt+1)),ca=Math.cos(A),sa=Math.sin(A),cb=Math.cos(B),sb=Math.sin(B);
      function pr(x,y,z){var X=x-IW/2,Y=y-8,x1=X*ca+z*sa,z1=-X*sa+z*ca,y2=Y*cb-z1*sb,z2=Y*sb+z1*cb,s=CAM/(CAM-z2);return [ox+x1*U*s,oy+y2*U*s,z2];}
      var R=ic.polys.map(function(p){return {F:p.map(function(q){return pr(q[0],q[1],DEPTH/2)}),B:p.map(function(q){return pr(q[0],q[1],-DEPTH/2)})};});
      var yc=oy,q0=1e9,q1=-1e9,X0=1e9,X1=-1e9,Y0=1e9,Y1=-1e9;
      R.forEach(function(r){r.F.concat(r.B).forEach(function(p){var q=p[0]-(yc-p[1])*TAN;if(q<q0)q0=q;if(q>q1)q1=q;});});
      /* measurement frame: fixed for the asset, from its rest pose */
      var fw=IW*U*.62+h*.05,fh=8*U+h*.075,fx0=ox-fw,fx1=ox+fw,fy0=h*.355-fh,fy1=h*.355+fh;api.frame={x0:fx0,x1:fx1,y0:fy0,y1:fy1};
      var pad=h*.08;
      function F(t0,t1){return lerp(q0-pad,q1+pad,eio((tt-t0)/(t1-t0)));}
      function region(fq,left){var x0=fq+(yc+h)*TAN,x1=fq+(yc-2*h)*TAN;c.beginPath();
        if(left){c.moveTo(-w,-h);c.lineTo(x0,-h);c.lineTo(x1,2*h);c.lineTo(-w,2*h);}else{c.moveTo(x0,-h);c.lineTo(3*w,-h);c.lineTo(3*w,2*h);c.lineTo(x1,2*h);}c.closePath();}
      var on=tt>=W0,gone=tt>=WO1,hold=c01((tt-LOCK)/1)*(1-c01((tt-O0)/.6));
      /* 1. grid and measurement frame */
      var gIn=eio((tt-.2)/1.0),ga=on?.075*gIn*(1-.7*c01((tt-LOCK-.3)/1.4))*(1-c01((tt-WO1+.6)/.6)):0;
      if(ga>.002){c.save();c.beginPath();c.rect(fx0,fy0,fx1-fx0,fy1-fy0);c.clip();c.strokeStyle=rgba(PAL.fg,ga);c.lineWidth=hair;c.beginPath();
        var st=U*4,xr=fx0+(fx1-fx0)*gIn;
        for(var gx=fx0-h*.3;gx<=xr;gx+=st){c.moveTo(Math.round(gx+(yc-fy0)*TAN)+hp,fy0);c.lineTo(Math.round(gx+(yc-fy1)*TAN)+hp,fy1);}
        for(var gy=fy0;gy<=fy1;gy+=st){var yy=Math.round(gy)+hp;c.moveTo(fx0,yy);c.lineTo(xr,yy);}
        c.stroke();c.restore();}
      var fa=on?eOut((tt-.3)/.6)*(1-c01((tt-WO1+.4)/.4)):0,bl=h*.03;
      if(fa>0){c.strokeStyle=rgba(PAL.fg,.45*fa);c.lineWidth=hair;c.beginPath();
        [[fx0,fy0,1,1],[fx1,fy0,-1,1],[fx0,fy1,1,-1],[fx1,fy1,-1,-1]].forEach(function(q){var x=Math.round(q[0])+hp,y=Math.round(q[1])+hp;c.moveTo(x+q[2]*bl,y);c.lineTo(x,y);c.lineTo(x,y+q[3]*bl);});c.stroke();
        /* ruler along the top edge, one tick per 4 units of the gun */
        c.strokeStyle=rgba(PAL.fg,.28*fa);c.beginPath();var rx0=ox-IW*U/2,rx1=ox+IW*U/2,ry=Math.round(fy0+h*.022)+hp;
        c.moveTo(Math.round(rx0)+hp,ry);c.lineTo(Math.round(rx1)+hp,ry);for(var u=0;u<=IW;u+=4){var tx=Math.round(rx0+u*U)+hp;c.moveTo(tx,ry-(u%16?h*.004:h*.009));c.lineTo(tx,ry);}c.stroke();
        c.font=Math.round(h*.015)+'px "UGG Mono"';c.textAlign='left';c.fillStyle=rgba(PAL.fg,.5*fa);c.fillText(IW+' U',Math.round(rx1)+h*.012,ry+h*.005);
        /* the mock label */
        c.fillStyle=rgba(PAL.fg,.55*fa);c.fillText('MOCK • REAL MODEL IN GAME',fx0,fy0-h*.018);
        c.strokeStyle=rgba(PAL.fg,.45*fa);c.setLineDash([3*d,3*d]);var mw=c.measureText('MOCK • REAL MODEL IN GAME').width;c.strokeRect(Math.round(fx0-h*.008)+hp,Math.round(fy0-h*.042)+hp,Math.round(mw+h*.016),Math.round(h*.034));c.setLineDash([]);}
      /* 2. bloom behind, on lock */
      var lp=(tt-LOCK)/.9,blm=on&&!gone?(.12*Math.exp(-Math.max(0,lp)*2.2)*c01(lp*6)+.045*hold):0;
      if(blm>.002){c.save();c.translate(ox,oy);c.scale(1.6,1);var rr=h*.36,rg=c.createRadialGradient(0,0,0,0,0,rr);
        for(var k=0;k<=8;k++){var r=k/8;rg.addColorStop(r,rgba(PAL.acc,(blm*Math.exp(-r*r*3.2)*(1-r)).toFixed(4)));}c.fillStyle=rg;c.fillRect(-rr,-rr,2*rr,2*rr);c.restore();}
      if(!on||gone)return readout();
      var fW=tt<WO0?F(W0,W1):null,fWo=tt>=WO0?F(WO0,WO1):null,fF=tt<O0?F(F0,F1):null,fFo=tt>=O0?F(O0,O1):null;
      function path(pts){c.moveTo(pts[0][0],pts[0][1]);for(var i=1;i<pts.length;i++)c.lineTo(pts[i][0],pts[i][1]);c.closePath();}
      /* 3. wire: front and back outlines and the edges between them */
      var wa=tt<LOCK?1:tt<O0?1-c01((tt-LOCK)/.6):c01((tt-O0)/.3);
      if(wa>0){c.save();if(fW!=null){region(fW,true);c.clip();}else{region(fWo,false);c.clip();}
        c.strokeStyle=rgba(PAL.fg,.6*wa);c.lineWidth=hair;c.beginPath();
        R.forEach(function(r){path(r.F);path(r.B);for(var i=0;i<r.F.length;i++){c.moveTo(r.F[i][0],r.F[i][1]);c.lineTo(r.B[i][0],r.B[i][1]);}});c.stroke();c.restore();}
      /* 4. body: back face, walls far to near, front face; filled behind the scan */
      var fillOn=(fF!=null&&tt>=F0)||(fFo!=null);
      if(fillOn){c.save();if(fF!=null){region(fF,true);c.clip();}else{region(fFo,false);c.clip();}
        c.beginPath();R.forEach(function(r){path(r.B)});c.fillStyle=rgba(PAL.fg,.16);c.fill('evenodd');
        var walls=[];R.forEach(function(r){var n=r.F.length;for(var i=0;i<n;i++){var i2=(i+1)%n,f0=r.F[i],f1=r.F[i2],b0=r.B[i],b1=r.B[i2];
          var ex=f1[0]-f0[0],ey=f1[1]-f0[1],L=Math.hypot(ex,ey)||1,lum=Math.abs((-ey*.55+ex*-.83)/L);walls.push({q:[f0,f1,b1,b0],z:(f0[2]+f1[2]+b0[2]+b1[2])/4,l:lum});}});
        walls.sort(function(p,q){return p.z-q.z;});
        walls.forEach(function(wl){c.beginPath();path(wl.q);c.fillStyle=rgba(PAL.fg,(.26+.34*wl.l).toFixed(3));c.fill();});
        var gg=c.createLinearGradient(0,oy-8*U,0,oy+8*U);gg.addColorStop(0,rgba(PAL.fg,.96));gg.addColorStop(1,rgba(PAL.fg,.74));
        c.beginPath();R.forEach(function(r){path(r.F)});c.fillStyle=gg;c.fill('evenodd');
        /* bevel: a bright hairline on the front outline */
        c.strokeStyle=rgba(PAL.fg,.95);c.lineWidth=Math.max(1,1.2*d);c.lineJoin='miter';c.stroke();
        /* 5. sheen across the front face */
        [6.0,10.4].forEach(function(s0){var p=(tt-s0)/2.8;if(p<=0||p>=1||tt>=O0)return;var k=Math.sin(Math.PI*p),cx=lerp(fx0-h*.2,fx1+h*.2,eio(p)),sg=c.createLinearGradient(cx-h*.16,0,cx+h*.16,0);
          [[0,0],[.25,.02],[.42,.09],[.49,.3],[.5,.4],[.51,.3],[.58,.09],[.75,.02],[1,0]].forEach(function(s){sg.addColorStop(s[0],rgba(PAL.fg,(s[1]*k).toFixed(4)));});
          c.save();c.beginPath();R.forEach(function(r){path(r.F)});c.clip('evenodd');c.transform(1,0,-TAN,1,TAN*yc,0);c.fillStyle=sg;c.fillRect(cx-h*.3,oy-h*.4,h*.6,h*.8);c.restore();});
        /* lock: outline flash with glow */
        if(lp>=0&&lp<1&&tt<O0){c.beginPath();R.forEach(function(r){path(r.F)});c.strokeStyle=rgba(PAL.acc,(.95*(1-lp)*(1-lp)).toFixed(3));c.lineWidth=Math.max(1.5,1.6*d);
          c.shadowColor=rgba(PAL.acc,.8*(1-lp));c.shadowBlur=14*d*(1-lp);c.stroke();c.shadowBlur=0;}
        c.restore();}
      /* the scan line and its wake */
      var sc=tt>=W0&&tt<W1+.05?[fW,(tt-W0)/(W1-W0)]:(tt>=WO0&&tt<WO1?[fWo,(tt-WO0)/(WO1-WO0)]:(tt>=O0&&tt<O1?[F(O0,O1),(tt-O0)/(O1-O0)]:null));
      if(sc){var al=Math.sin(Math.PI*c01(sc[1])),sx=sc[0],yT=fy0,yB=fy1,wk=h*.14;
        /* the scan and its wake stay inside the measurement frame */
        c.save();c.beginPath();c.rect(fx0,fy0,fx1-fx0,fy1-fy0);c.clip();c.beginPath();c.moveTo(sx+(yc-yT)*TAN,yT);c.lineTo(sx+(yc-yB)*TAN,yB);c.lineTo(sx+(yc-yB)*TAN-wk,yB);c.lineTo(sx+(yc-yT)*TAN-wk,yT);c.closePath();
        var wg=c.createLinearGradient(sx-wk,0,sx,0);wg.addColorStop(0,rgba(PAL.acc,0));wg.addColorStop(1,rgba(PAL.acc,.15*al));c.fillStyle=wg;c.fill();
        c.strokeStyle=rgba(PAL.acc,al);c.lineWidth=Math.max(1.5,1.6*d);c.beginPath();c.moveTo(sx+(yc-yT)*TAN,yT);c.lineTo(sx+(yc-yB)*TAN,yB);c.stroke();c.restore();}
      /* 6. label plate, wiping in on the slant; spec ticks snapped to device pixels */
      var lw=eOut((tt-LB0)/(LB1-LB0))*(1-eIn((tt-LBO)/.35));
      if(lw>0){var px0=Math.round(w*.49),py0=Math.round(h*.575),pw=Math.round(w*.33),ph=Math.round(h*.215),sk=ph*TAN;
        c.save();c.beginPath();c.moveTo(px0,py0);c.lineTo(px0+pw*lw+sk,py0);c.lineTo(px0+pw*lw,py0+ph);c.lineTo(px0,py0+ph);c.closePath();c.clip();
        c.fillStyle=rgba(PAL.ink,.72);c.fillRect(px0,py0,pw,ph);c.strokeStyle=rgba(PAL.fg,.16);c.lineWidth=hair;c.strokeRect(px0+hp,py0+hp,pw-hair,ph-hair);
        c.fillStyle=rgba(PAL.acc,1);c.fillRect(px0,py0,Math.max(2,Math.round(3*d)),ph);
        var tx=px0+Math.round(h*.028);c.textAlign='left';c.fillStyle=rgba(PAL.fg,1);c.font='700 '+Math.round(h*.046)+'px "UGG Display"';c.fillText(a.n,tx,py0+ph*.3);
        c.font=Math.round(h*.0155)+'px "UGG Mono"';c.fillStyle=rgba(PAL.fg,.6);c.fillText(a.g+' • '+CLS[a.k].toUpperCase()+' • RUNG '+pad2(a.r||0),tx,py0+ph*.44);
        /* real numbers first, in UGG Mono; a pixel-snapped tick bar beside each as a secondary visual */
        var rpm=Math.round(60/a.cyc),pr=[['RATE',rpm+' RPM',rpm/1000],['DAMAGE',String(a.dmg),a.dmg/120],['MAG',String(a.mag),a.mag/60]],grow=c01((tt-LB1)/.5);
        pr.forEach(function(sp,ri){var yy=Math.round(py0+ph*(.6+ri*.115)),n=20,bx=Math.round(tx+h*.2),bw=Math.round(pw-(bx-px0)-h*.03),pit=Math.max(2,Math.floor(bw/n)),tk=Math.max(1,Math.round(pit*.4)),th=Math.max(2,Math.round(h*.009)),lit=Math.round(n*Math.min(1,sp[2])*grow);
          c.font=Math.round(h*.0145)+'px "UGG Mono"';c.fillStyle=rgba(PAL.fg,.45);c.fillText(sp[0],tx,yy+th);c.fillStyle=rgba(PAL.fg,1);c.fillText(sp[1],tx+h*.075,yy+th);
          for(var i=0;i<n;i++){c.fillStyle=i<lit?rgba(PAL.fg,.55):rgba(PAL.fg,.12);c.fillRect(bx+i*pit,yy,tk,th);}});
        c.font=Math.round(h*.0115)+'px "UGG Mono"';c.fillStyle=a.src.indexOf('SAMPLE')===0?rgba(PAL.acc,1):rgba(PAL.fg,.38);c.fillText(a.src,tx,py0+ph*.95);
        c.restore();}
      readout();
      function readout(){
        var stx=!on||gone?'STANDBY':tt<LOCK?'MATERIALIZING':tt<O0?'LOCKED':'SIGNING OFF',lk=stx==='LOCKED',ry=fy1+h*.045,sq=Math.round(h*.013);
        c.font=Math.round(h*.017)+'px "UGG Mono"';c.textAlign='left';
        if(lk){c.fillStyle=rgba(PAL.acc,1);c.fillRect(Math.round(fx0),Math.round(ry-sq),sq,sq);}else{c.strokeStyle=rgba(PAL.fg,.5);c.lineWidth=hair;c.strokeRect(Math.round(fx0)+hp,Math.round(ry-sq)+hp,sq-hair,sq-hair);}
        c.fillStyle=rgba(PAL.fg,lk?.7:.45);c.fillText('SHOWCASE • '+stx,fx0+sq+h*.012,ry);
        c.textAlign='right';c.fillStyle=rgba(PAL.fg,.4);c.fillText('ASSET '+pad2(j+1)+' / '+pad2(SH.length),fx1,ry);
      }
    }};
    return api;
  }

  /* ------------------------------------------------ menu ambient: one fragment shader, with a 2D fallback */
  /* one ambient for every between-match screen; o.col: right edge of a calm column (or -1), o.bloom: bloom centre (x, y up) */
  function Ambient(el,o){
    o=o||{col:.30,bloom:[.68,.56],menu:true};
    var cg=$(el,'canvas.amb'),c2=document.createElement('canvas'),mode='gl',gl=null,prog=null,U={},why='',st=o.menu?document.querySelector('.amb-st'):null;
    c2.className='amb amb2';c2.setAttribute('aria-hidden','true');c2.style.display='none';cg.parentNode.insertBefore(c2,cg.nextSibling);
    var SRC=(document.getElementById('amb-src')||{}).textContent||'',COL=o.col;
    var VS='attribute vec2 p;void main(){gl_Position=vec4(p,0.,1.);}';
    function initGL(){
      try{gl=cg.getContext('webgl',{alpha:false,antialias:false,depth:false,stencil:false,preserveDrawingBuffer:true,powerPreference:'low-power'})||cg.getContext('experimental-webgl');}catch(e){gl=null;}
      if(!gl){why='no WebGL';return false;}
      function sh(type,src){var s=gl.createShader(type);gl.shaderSource(s,src);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS)){why='compile: '+gl.getShaderInfoLog(s);return null;}return s;}
      var v=sh(gl.VERTEX_SHADER,VS),f=sh(gl.FRAGMENT_SHADER,SRC);if(!v||!f)return false;
      prog=gl.createProgram();gl.attachShader(prog,v);gl.attachShader(prog,f);gl.linkProgram(prog);
      if(!gl.getProgramParameter(prog,gl.LINK_STATUS)){why='link failed';return false;}
      gl.useProgram(prog);
      var b=gl.createBuffer();gl.bindBuffer(gl.ARRAY_BUFFER,b);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-1,-1,3,-1,-1,3]),gl.STATIC_DRAW);
      var a=gl.getAttribLocation(prog,'p');gl.enableVertexAttribArray(a);gl.vertexAttribPointer(a,2,gl.FLOAT,false,0,0);
      ['uRes','uTime','uInk','uFg','uAcc','uCol','uBloom'].forEach(function(n){U[n]=gl.getUniformLocation(prog,n)});
      return true;
    }
    var glOK=/[?&]gl=0/.test(location.search)?(why='disabled by ?gl=0',false):initGL();
    cg.addEventListener('webglcontextlost',function(ev){ev.preventDefault();glOK=false;why='context lost';setMode('2d');});
    function n3(c){return [c[0]/255,c[1]/255,c[2]/255]}
    function size(cv,d){var W=cv.clientWidth||el.clientWidth,H=cv.clientHeight||el.clientHeight;if(!W)return false;var w=Math.round(W*d),h=Math.round(H*d);if(cv.width!==w||cv.height!==h){cv.width=w;cv.height=h;}return true;}
    function drawGL(t){
      if(!size(cg,Math.min(1.5,DPR())))return;
      gl.viewport(0,0,cg.width,cg.height);
      gl.uniform2f(U.uRes,cg.width,cg.height);gl.uniform1f(U.uTime,t);
      gl.uniform3fv(U.uInk,n3(PAL.ink));gl.uniform3fv(U.uFg,n3(PAL.fg));gl.uniform3fv(U.uAcc,n3(PAL.acc));gl.uniform1f(U.uCol,COL);gl.uniform2f(U.uBloom,o.bloom[0],o.bloom[1]);
      gl.drawArrays(gl.TRIANGLES,0,3);
    }
    /* 2D: the same composition with gradients and polygons; the slow and the live field blend at the menu edge */
    function fr(x){return x-Math.floor(x)}
    function hash(x,y){x=fr(x*123.34);y=fr(y*456.21);var k=x*(x+45.32)+y*(y+45.32);x+=k;y+=k;return fr(x*y);}
    function smooth(x){x=c01(x);return x*x*(3-2*x)}
    var offA=document.createElement('canvas'),offB=document.createElement('canvas');
    function field(cv,W,H,t,bf,fa){
      cv.width=W;cv.height=H;var c=cv.getContext('2d');c.clearRect(0,0,W,H);c.globalCompositeOperation='lighter';
      var span=W+H*TAN,g=c.createLinearGradient(0,0,span/(1+TAN*TAN),span/(1+TAN*TAN)*TAN);
      for(var i=0;i<=32;i++){var f=i/32,s=(f*span-H*TAN)/H;
        var b=.5+.25*Math.sin(2*Math.PI*(s*.55-t*.011))+.25*Math.sin(2*Math.PI*(s*1.3-t*.017)+1.7);
        g.addColorStop(f,rgba(PAL.fg,(b*.022*.85).toFixed(4)));}
      c.fillStyle=g;c.fillRect(0,0,W,H);
      var smin=-TAN,smax=W/H,SX=2.2,SY=1.6,OX=.37,OY=.29;
      for(var iy=0;iy<3;iy++)for(var ix=Math.floor(smin*SX+OX)-1;ix<=Math.ceil(smax*SX+OX);ix++)for(var tr=0;tr<2;tr++){
        var kx=ix*2+tr,ky=iy*2,ph=hash(kx+7.1,ky+7.1),e=t/24+ph,k=Math.floor(e),wv=smooth(fr(e));
        var v=lerp(hash(kx+k*13.7,ky+k*13.7),hash(kx+(k+1)*13.7,ky+(k+1)*13.7),wv),val=(v-.5)*.018*fa;
        var Q=tr?[[1,0],[1,1],[0,1]]:[[0,0],[1,0],[0,1]],cxs=0,cys=0;
        c.beginPath();Q.forEach(function(q,j){var s=(ix+q[0]-OX)/SX,yu=(iy+q[1]-OY)/SY,x=(s+yu*TAN)*H,y=H-yu*H;cxs+=x;cys+=y;if(j)c.lineTo(x,y);else c.moveTo(x,y);});c.closePath();
        var bl=bf(cxs/3,cys/3),a=val*(.75+1.6*bl);
        if(a>0){c.globalCompositeOperation='lighter';c.fillStyle=rgba(PAL.fg,a.toFixed(4));}else{c.globalCompositeOperation='source-over';c.fillStyle='rgba(0,0,0,'+(-a*3).toFixed(4)+')';}
        c.fill();
      }
      c.globalCompositeOperation='source-over';
    }
    function draw2D(t){
      if(!size(c2,1))return;var W=c2.width,H=c2.height,c=c2.getContext('2d');
      var cx=W*(o.bloom[0]+.025*Math.sin(t*.043)),cy=H-H*(o.bloom[1]+.03*Math.sin(t*.031+1));
      function bf(x,y){var dx=(x-cx)/H*.85,dy=(y-cy)/H*1.15;return Math.exp(-(dx*dx+dy*dy)*2.6);}
      c.globalCompositeOperation='source-over';c.fillStyle=rgba(PAL.ink,1);c.fillRect(0,0,W,H);
      field(offB,W,H,t*.06,bf,.4);field(offA,W,H,t,bf,1);
      var oa=offA.getContext('2d'),m=oa.createLinearGradient(W*(COL-.04),0,W*(COL+.2),0);m.addColorStop(0,'rgba(0,0,0,0)');m.addColorStop(1,'rgba(0,0,0,1)');
      oa.globalCompositeOperation='destination-in';oa.fillStyle=m;oa.fillRect(0,0,W,H);oa.globalCompositeOperation='source-over';
      var ob=offB.getContext('2d'),m2=ob.createLinearGradient(W*(COL-.04),0,W*(COL+.2),0);m2.addColorStop(0,'rgba(0,0,0,1)');m2.addColorStop(1,'rgba(0,0,0,0)');
      ob.globalCompositeOperation='destination-in';ob.fillStyle=m2;ob.fillRect(0,0,W,H);ob.globalCompositeOperation='source-over';
      c.drawImage(offB,0,0);c.drawImage(offA,0,0);
      c.globalCompositeOperation='lighter';
      c.save();c.translate(cx,cy);c.scale(1/.85,1/1.15);var R=H*1.4,rg=c.createRadialGradient(0,0,0,0,0,R);
      for(var i=0;i<=12;i++){var r=i/12*R/H;rg.addColorStop(i/12,rgba(PAL.acc,(Math.exp(-r*r*2.6)*.15).toFixed(4)));}
      c.fillStyle=rg;c.fillRect(-R,-R,2*R,2*R);c.restore();
      var lb=c.createLinearGradient(0,H,0,H*.65);lb.addColorStop(0,rgba(PAL.acc,.025));lb.addColorStop(1,rgba(PAL.acc,0));c.fillStyle=lb;c.fillRect(0,H*.6,W,H*.4);
      c.globalCompositeOperation='source-over';
      c.save();c.translate(W/2,H/2);c.scale(W/H,1);var vg=c.createRadialGradient(0,0,0,0,0,H*.7071);
      for(var j=0;j<=8;j++){var q=j/8*.7071;vg.addColorStop(j/8,'rgba(0,0,0,'+(.35*q*q).toFixed(4)+')');}
      c.fillStyle=vg;c.fillRect(-H,-H,2*H,2*H);c.restore();
    }
    function status(){
      if(!st)return;
      var txt=mode==='gl'?'Ambient • WebGL fragment shader • '+(RM?'still frame (reduced motion)':'live'):mode==='2d'?'Ambient • 2D canvas fallback'+(glOK?' (chosen)':' • WebGL unavailable: '+why):'Ambient • off';
      st.innerHTML='<i'+(mode==='off'?' class="off"':'')+'></i>'+esc(txt);
    }
    function setMode(m){
      if(m==='gl'&&!glOK)m='2d';mode=m;
      cg.style.display=m==='gl'?'':'none';c2.style.display=m==='2d'?'':'none';el.classList.toggle('amb-off',m==='off');
      if(o.menu)$$(document,'.ctl.amm').forEach(function(b){var on=b.getAttribute('data-m')===m;b.setAttribute('aria-checked',on?'true':'false');b.tabIndex=on?0:-1;});
      last=-1;status();
    }
    var last=-1,lastKey='';
    var api={mode:function(){return mode},set:setMode,why:function(){return why},glOK:function(){return glOK},
      draw:function(T){
        var t=RM?40:T+40,key=PALKEY+'|'+(el.clientWidth);
        if(RM&&t===last&&key===lastKey)return;last=t;lastKey=key;
        if(mode==='gl')drawGL(t);else if(mode==='2d')draw2D(t);
      },redraw:function(){last=-1;}};
    setMode(glOK?'gl':'2d');
    if(!glOK&&o.menu)$$(document,'.ctl.amm[data-m=gl]').forEach(function(b){b.disabled=true;b.title='WebGL unavailable: '+why;});
    return api;
  }

  add('#fr-menu',function(s){
    var el=s.el,cv=$(el,'canvas.bd'),ctx=cv.getContext('2d'),IDENT=Ident(),SHOW=SHOW_API=Showcase(),C=[LADDER_API=Ladder(),Assembly(),{still:SHOW.still,draw:function(){var x=SHOWMODE==='assets'?SHOW:IDENT;return x.draw.apply(x,arguments);}}],cur=0,prev=-1,swT=-9,base=[0,0,0],AMB=Ambient(el);
    var tabs=$$(el,'.ctabs [role=tab]'),cap=$(el,'.cap'),pgs=tabs.map(function(b){return $(b,'.pg')}),DWELLS=[16,16,16],autoOn=!RM,t0=0;
    var CAPS=[['A • Ladder','Fourteen rungs, one gun per kill, knife at the top'],['B • Assembly','Guns from every game, built line by line'],['C • Showcase','Assets materialize on the scan; the Ident when there are none']];
    var autoB=document.querySelector('.ctl.auto'),iconB=$$(document,'.ctl.icm'),ambB=$$(document,'.ctl.amm');
    s.still=0;s.amb=AMB;
    function capT(){var x=CAPS[cur][1];if(cur===0&&ICONMODE==='dl')x='Downloaded icons (mock) • CA: fallback';if(cur===2&&SHOWMODE!=='assets')x='No assets: the Ident, the logo signing on';cap.innerHTML='<b>'+CAPS[cur][0]+'</b>'+x;}
    function choose(i,byUser){
      if(byUser)t0=s.t;
      if(i===cur)return; prev=cur;cur=i;swT=s.t;t0=s.t;base[i]=RM?-C[i].still:s.t;
      tabs.forEach(function(b,j){b.setAttribute('aria-selected',j===i?'true':'false');b.tabIndex=j===i?0:-1;});
      capT();
      if(RM)s.update(s.t);
    }
    tabs.forEach(function(b,i){b.tabIndex=i===0?0:-1;});
    radios(null,tabs,function(b,i){choose(i,true)});
    base[0]=RM?-C[0].still:0;
    s.onStart=function(){base[cur]=0;prev=-1;t0=0;};
    s.onPal=function(){AMB.redraw();};
    /* for captures: show backdrop i from its own t = 0, auto-swap off */
    s.pick=function(i){choose(i,true);base[i]=RM?-C[i].still:0;prev=-1;autoOn=false;if(autoB)autoB.setAttribute('aria-pressed','false');};
    if(autoB){if(RM){autoB.setAttribute('aria-pressed','false');autoB.disabled=true;autoB.title='Off with reduced motion';}
      autoB.addEventListener('click',function(){autoOn=!autoOn;t0=s.t;autoB.setAttribute('aria-pressed',autoOn?'true':'false');});}
    iconB.forEach(function(b){b.addEventListener('click',function(){ICONMODE=b.getAttribute('data-m');iconB.forEach(function(x){x.setAttribute('aria-checked',x===b?'true':'false')});
      if(cur!==0)choose(0,true);capT();if(RM)s.update(s.t);});});
    radios(null,ambB,function(b){AMB.set(b.getAttribute('data-m'));s.update(s.t);});
    var shB=$$(document,'.ctl.shm');shB.forEach(function(b,i){b.tabIndex=i===0?0:-1;});
    radios(null,shB,function(b){SHOWMODE=b.getAttribute('data-m');shB.forEach(function(x){var o=x===b;x.setAttribute('aria-checked',o?'true':'false');x.tabIndex=o?0:-1;});
      C[2].still=SHOWMODE==='assets'?SHOW.still:IDENT.still;if(cur!==2)choose(2,true);else{base[2]=RM?-C[2].still:s.t;}capT();s.update(s.t);});
    /* the backdrop fades out toward the menu column, so the ambient shows through behind the menu */
    function fadeLeft(w,h){
      ctx.globalCompositeOperation='destination-out';
      var g=ctx.createLinearGradient(0,0,w,0);g.addColorStop(0,'rgba(0,0,0,1)');g.addColorStop(.27,'rgba(0,0,0,.92)');g.addColorStop(.46,'rgba(0,0,0,0)');
      ctx.fillStyle=g;ctx.fillRect(0,0,w,h);ctx.globalCompositeOperation='source-over';
    }
    s.update=function(T){
      AMB.draw(T);
      var DWELL=DWELLS[cur]=cur===0?LADDER_API.period():16;if(autoOn&&!RM&&T-t0>=DWELL)choose((cur+1)%3,false);
      pgs.forEach(function(p,j){setS(p,'transform','scaleX('+(j===cur&&autoOn&&!RM?c01((T-t0)/DWELL):0).toFixed(3)+')');});
      var d=Math.min(2,DPR()),W=cv.clientWidth,H=cv.clientHeight;if(!W)return;
      if(cv.width!==Math.round(W*d)||cv.height!==Math.round(H*d)){cv.width=Math.round(W*d);cv.height=Math.round(H*d);}
      var w=cv.width,h=cv.height;ctx.setTransform(1,0,0,1,0,0);ctx.clearRect(0,0,w,h);
      var p=(T-swT)/.52;
      if(prev>=0&&p>=0&&p<1&&!RM){
        C[prev].draw(ctx,w,h,T-base[prev],d,T);
        var e=eio(p),ex=lerp(-h*TAN,w*1.02,e),sk=h*TAN;
        ctx.save();ctx.beginPath();ctx.moveTo(0,0);ctx.lineTo(ex+sk,0);ctx.lineTo(ex,h);ctx.lineTo(0,h);ctx.closePath();ctx.clip();
        ctx.clearRect(0,0,w,h);C[cur].draw(ctx,w,h,T-base[cur],d,T);ctx.restore();
        ctx.fillStyle=rgba(PAL.acc,1);ctx.beginPath();ctx.moveTo(ex+sk,0);ctx.lineTo(ex+sk+3*d,0);ctx.lineTo(ex+3*d,h);ctx.lineTo(ex,h);ctx.fill();
      } else C[cur].draw(ctx,w,h,T-base[cur],d,T);
      fadeLeft(w,h);
    };
  });

  /* ================================================= 02 LOBBY */
  add('#fr-lobby',function(s){
    var el=s.el,P=20;s.still=8.2;
    var cdv=$(el,'.cdv'),cdt=$(el,'.cd .tag'),rdy=$(el,'.rdy'),mm=$(el,'.lrow.mm'),tb=new TickBar($(el,'canvas.tickbar')),pass=Pass(el),last={};
    tickbars.push(tb);
    s.update=function(T){
      var t=mod(T,P),left=Math.max(0,Math.ceil(19-t));
      setT(cdv,'00:'+pad2(left));setT(cdt,t>=19?'Starting':'Starts in');
      var n=tb.n,lit=Math.min(n,Math.floor(t/19*n));
      tb.draw(t>=19?n:lit,t>=19?0:Math.floor(16/19*n));
      var r=t>=7;if(last.mm!==r){last.mm=r;mm.classList.toggle('nr',!r);mm.classList.toggle('flip',r);setT($(mm,'.st-t'),r?'Ready':'Not ready');setT(rdy,r?'6 / 7':'5 / 7');}
      pass.set(RM?-1:mod(t-19.2,P)/1.2);
    };
  });

  /* ================================================= 03 FLOW */
  add('#fr-flow',function(s){
    var el=s.el,SEG=3.6,HOLD=2.4,P=4*SEG,scr=$$(el,'.scr'),st=$$(el,'.steps span'),pass=Pass(el),cur=-1;s.still=1.2;
    s.update=function(T){
      var t=mod(T,P),i=mod(Math.floor((t-HOLD-.6)/SEG)+1,4);
      if(i!==cur){cur=i;scr.forEach(function(x,j){x.classList.toggle('on',j===i)});st.forEach(function(x,j){x.classList.toggle('on',j===i)});}
      var lt=mod(t,SEG);pass.set(RM?-1:(lt>=HOLD?(lt-HOLD)/1.2:-1));
    };
  });

  /* ================================================= 04 HUD: chip, and the board while Tab is held */
  add('#fr-hud',function(s){
    var el=s.el,P=9;s.still=4.6;
    var disc=$(el,'.disc'),C0=$(disc,'.c0:not(.cl)'),CL=$(disc,'.c0.cl'),B0=$(disc,'.c2.ch'),BL=$(disc,'.c2.lw'),hk=$(el,'.holdk'),man=null,lead=false;
    var S0=[21,3.05],S2=[64,25.9];
    var amv=$(el,'.am .v'),am=$(el,'.am'),ab=$$(el,'.am .bars i'),xh=$(el,'.xh'),lastA=-1;
    var KF=$$(el,'.kf .e').map(function(r){return {el:r,t:+r.getAttribute('data-t')}}),LIFE=5,RH=2.15;
    var modeB=$$(document,'.ctl.ldm');
    /* two states: press opens the board (240 ms), release folds it back (200 ms) */
    function geo(t,d,u){
      var g={w:S0[0],h:S0[1],o0:1,o2:0,k2:-9,down:false};
      if(d==null||t<d)return g;
      var a=eOut((t-d)/.24);
      g.w=lerp(S0[0],S2[0],a);g.h=lerp(S0[1],S2[1],a);g.o0=1-c01((t-d)/.08);g.o2=c01((t-d-.06)/.12);g.k2=t-d-.08;g.down=true;
      if(u!=null&&t>=u){var gu=geo(u,d,null),cp=eio((t-u)/.2),fo=1-c01((t-u)/.07);
        g.w=lerp(gu.w,S0[0],cp);g.h=lerp(gu.h,S0[1],cp);g.o2=gu.o2*fo;g.o0=c01((t-u-.12)/.1);g.k2=gu.k2;g.down=false;}
      return g;
    }
    function rows(R,k,st){R.forEach(function(r,i){var p=c01((k-i*st)/.16);setS(r,'opacity',p.toFixed(3));setS(r,'transform',p<1?'translateY('+((1-eOut(p))*.45).toFixed(3)+'cqw)':'');});}
    var R0=[$(B0,'.top')].concat($$(B0,'.sbr')),RL=[$(BL,'.top')].concat($$(BL,'.sbr'));
    s.update=function(T){
      var t,d,u;
      if(man){t=T;d=man.d;u=man.u;}else{t=mod(T,P);d=4.2;u=7.4;}
      var g=geo(t,d,u),ch=lead?CL:C0,bo=lead?BL:B0,other=lead?C0:CL,ob=lead?B0:BL;
      setS(disc,'width',(g.w+(lead?2*(1-c01((g.w-S0[0])/4)):0)).toFixed(3)+'cqw');setS(disc,'height',g.h.toFixed(3)+'cqw');
      setS(ch,'opacity',g.o0.toFixed(3));setS(other,'opacity','0');setS(ob,'opacity','0');setS(ob,'visibility','hidden');
      setS(bo,'opacity',g.o2.toFixed(3));setS(bo,'visibility',g.o2>0?'visible':'hidden');
      rows(lead?RL:R0,g.k2,.025);
      hk.classList.toggle('down',g.down);
      /* kill feed: newest at the bottom; each row lives 5 s, older rows close the gap */
      var ta=mod(T,P);
      KF.forEach(function(k,i){
        var age=ta-k.t,inn=eOut(age/.22),out=c01((age-LIFE)/.25),y=0;
        for(var j=0;j<i;j++){var aj=ta-KF[j].t;if(aj>=0)y+=1-eio(c01((aj-LIFE)/.25));}
        var vis=age>=0&&out<1;
        setS(k.el,'opacity',vis?(1-out).toFixed(3):'0');
        setS(k.el,'transform','translate('+((1-inn)*2.4).toFixed(3)+'cqw,'+(y*RH).toFixed(3)+'cqw)');
      });
      /* ammo and crosshair, on the auto loop */
      var ammo=30,spread=0;
      [[.8,6],[2.0,5],[5.6,6]].forEach(function(b){var k=clamp(Math.floor((ta-b[0])*10)+1,0,b[1]);ammo-=k;var e=b[0]+b[1]/10;
        if(ta>=b[0]&&ta<e)spread=Math.max(spread,1-((ta-b[0])%.1)/.1*.35);else if(ta>=e)spread=Math.max(spread,.65*(1-c01((ta-e)/.35)));});
      var rl=ta>=7.9&&ta<8.9;if(rl)ammo=Math.round(13+17*c01((ta-7.9)/1));
      if(lastA!==ammo){lastA=ammo;amv.textContent=ammo;ab.forEach(function(b,i){b.className=i<ammo?'on':''});}
      am.classList.toggle('reload',rl);
      xh.style.setProperty('--spread',(.55+.5*spread).toFixed(3)+'cqw');
    };
    function down(ev){if(ev)ev.preventDefault();man={d:RM?-10:s.t,u:null};if(RM)s.update(0);}
    function up(){if(man&&man.u==null){man.u=RM?-5:s.t;if(RM)s.update(0);}}
    hk.addEventListener('pointerdown',function(ev){down(ev);try{hk.setPointerCapture(ev.pointerId)}catch(e){}});
    hk.addEventListener('pointerup',up);hk.addEventListener('pointercancel',up);hk.addEventListener('lostpointercapture',up);
    hk.addEventListener('keydown',function(ev){if((ev.key===' '||ev.key==='Enter')&&!ev.repeat){down(ev);}else if(ev.key===' '||ev.key==='Enter')ev.preventDefault();});
    hk.addEventListener('keyup',function(ev){if(ev.key===' '||ev.key==='Enter'){ev.preventDefault();up();}});
    hk.addEventListener('click',function(ev){ev.preventDefault();});
    hk.addEventListener('contextmenu',function(ev){ev.preventDefault();});
    modeB.forEach(function(b,i){b.tabIndex=i===0?0:-1;});
    radios(null,modeB,function(b){lead=b.getAttribute('data-m')==='lead';modeB.forEach(function(x){x.setAttribute('aria-checked',x===b?'true':'false');x.tabIndex=x===b?0:-1;});s.update(RM?s.still:s.t);});
    s.onStart=function(){man=null;};
  });

  /* ================================================= 06 RANK UP */
  add('#fr-rank',function(s){
    var el=s.el,P=7.2;s.still=1.55;
    var toast=$(el,'.toast'),ra=$(toast,'.ra'),rb=$(toast,'.rb'),ga=$(toast,'.ga'),gb=$(toast,'.gb'),nxw=$(toast,'.nxw'),nx=$(toast,'.nx');
    var c0=$(el,'.c0'),yra=$(c0,'.yv .ra'),yrb=$(c0,'.yv .rb'),cgw=$(c0,'.gw'),mts=$$(c0,'.mt i'),kfs=[$(el,'.k1'),$(el,'.k2'),$(el,'.k3')],layer=$(el,'.hl');
    var E=[{t:.8,from:6,to:7,g:{k:'smg',raw:'MP5A4',short:'MP5A4',src:'Combat Arms'},nx:{k:'smg',raw:'P90'}},
           {t:3.1,from:7,to:8,g:{k:'smg',raw:'P90',src:'CS:S'},nx:{k:'rifle',raw:'M4A1 Carbine Desert Camo Champion Edition'}},
           {t:3.52,from:8,to:9,g:{k:'rifle',raw:'M4A1 Carbine Desert Camo Champion Edition',src:'CS:S'},nx:{k:'energy',raw:'AR2'}}];
    var HOLD_END=.92,EXIT=.24,last={};
    function gunHTML(g){return '<span class="ib">'+giHTML(g.k)+'</span><div class="nmw"><span class="l1"></span><span class="l2"></span></div>';}
    function fillGun(slot,g){if(slot._g===g)return;slot._g=g;slot.innerHTML=g?gunHTML(g):'';if(g)applyName($(slot,'.nmw'),fitName(g.raw,g.src,g.k,g.short));}
    function fillNext(x){if(nxw._g===x)return;nxw._g=x;nxw.innerHTML=giHTML(x.k)+'<b>'+esc(fitName(x.raw,'',x.k,x.short).base)+'</b>';}
    s.onFonts=function(){ga._g=gb._g=nxw._g=null;last={};};
    s.update=function(T){
      var t=mod(T,P),k=-1;for(var i=0;i<E.length;i++)if(E[i].t<=t)k=i;
      var la=t<.25?c01(t/.25):1-c01((t-6.85)/.25);setS(layer,'opacity',la.toFixed(3));
      kfs.forEach(function(r,i){var p=eOut((t-E[i].t)/.22);setS(r,'opacity',p>0?'1':'0');setS(r,'transform','translateX('+((1-p)*2.4).toFixed(3)+'cqw)');});
      if(k<0){
        setS(toast,'visibility','hidden');
        setT(yra,'06');setS(yra,'transform','');setS(yrb,'transform','translateY(110%)');
        if(last.ci!=='smg'){last.ci='smg';cgw.innerHTML=giHTML('smg');}
        mts.forEach(function(m,i){var c=(i+1<6?'on':i+1===6?'cur':'')+(i+1===11?' ld':'');if(m.className!==c)m.className=c;setS(m,'scale','');});
        return;
      }
      var j=k;while(j>0&&E[j].t<E[j-1].t+HOLD_END+EXIT)j--;
      var e=E[k],dt=t-e.t,en=eOut((t-E[j].t)/.2),ex=eIn((dt-HOLD_END)/EXIT);
      setS(toast,'visibility',(en>0&&ex<1)?'visible':'hidden');
      var sk='.66cqw';
      setS(toast,'clipPath',ex>0?'polygon(calc('+(ex*100).toFixed(2)+'% + '+sk+') 0,calc(100% + '+sk+') 0,100% 100%,'+(ex*100).toFixed(2)+'% 100%)'
                                 :'polygon(0 0,calc('+(en*100).toFixed(2)+'% + '+sk+') 0,'+(en*100).toFixed(2)+'% 100%,0 100%)');
      /* the number rolls upward: the old one leaves through the top, the new one rises from below */
      var q=c01((dt-.08)/.3),re=back(q,.9);
      setT(ra,pad2(e.from));setT(rb,pad2(e.to));
      setS(ra,'transform','translateY('+(-re*110).toFixed(2)+'%)');setS(ra,'opacity',(1-c01(q*1.6)*.7).toFixed(3));
      setS(rb,'transform','translateY('+((1-re)*110).toFixed(2)+'%)');
      setT(yra,pad2(e.from));setT(yrb,pad2(e.to));
      setS(yra,'transform','translateY('+(-re*110).toFixed(2)+'%)');setS(yrb,'transform','translateY('+((1-re)*110).toFixed(2)+'%)');
      var ck=q>.5?e.g.k:(k>0?E[k-1].g.k:'smg');if(last.ci!==ck){last.ci=ck;cgw.innerHTML=giHTML(ck);}
      mts.forEach(function(m,i){var r=i+1,c=(r<e.to?'on':r===e.to?'cur':'')+(r===11?' ld':'');if(m.className!==c)m.className=c;
        setS(m,'scale',r===e.to?'1 '+(1+.9*Math.sin(Math.PI*c01((dt-.08)/.3))).toFixed(3):'');});
      /* gun: wipes in; on a chain the previous gun lifts out first */
      if(k===j){fillGun(ga,null);fillGun(gb,e.g);var w=eOut((dt-.12)/.28);setS(gb,'clipPath','inset(0 '+((1-w)*100).toFixed(2)+'% 0 0)');setS(gb,'transform','');setS(gb,'opacity','1');}
      else{fillGun(ga,E[k-1].g);fillGun(gb,e.g);var o=c01(dt/.16);setS(ga,'transform','translateY('+(-eOut(o)*45).toFixed(2)+'%)');setS(ga,'opacity',(1-o).toFixed(3));
        var w2=eOut((dt-.07)/.28);setS(gb,'clipPath','inset(0 '+((1-w2)*100).toFixed(2)+'% 0 0)');}
      if(k===j){setS(ga,'opacity','0');}
      /* next gun */
      if(k===j){fillNext(e.nx);setS(nx,'opacity',eOut((dt-.2)/.25).toFixed(3));}
      else{var sw=dt<.12;fillNext(sw?E[k-1].nx:e.nx);setS(nx,'opacity',c01(Math.abs(dt-.12)/.12).toFixed(3));}
    };
  });

  /* ================================================= 07 AIR STRIKE */
  add('#fr-air',function(s){
    var el=s.el,P=8;s.still=1.0;
    var edge=$(el,'.edge'),ban=$(el,'.banner'),bt=$(el,'.bt'),bn=$(el,'.bn'),thr=$(el,'.chip.thr'),mg=$(el,'.chip.mg'),imn=$(el,'.imn'),ths=$(thr,'.s'),ring=$(el,'.ring .ar'),rv=$(el,'.ring b'),imp=$(el,'.imp'),map=$(el,'.map'),IMP=4.2;
    function pulse(x){return x<0?0:x<.08?x/.08:Math.exp(-(x-.08)/.32);}
    function chip(c,p,y){setS(c,'opacity',p>0?'1':'0');setS(c,'clipPath','inset(0 '+((1-p)*100).toFixed(2)+'% 0 0)');setS(c,'transform','translate('+(-(1-p)*1.2).toFixed(3)+'cqw,'+y.toFixed(3)+'cqw)');}
    s.update=function(T){
      var t=mod(T,P),n=t<IMP?Math.ceil(IMP-t):0,I=0;
      [.2,1.2,2.2,3.2].forEach(function(tp){I=Math.max(I,pulse(t-tp))});I=Math.max(I,1.3*pulse(t-IMP));
      var base=(t>.2&&t<IMP)?.2:0;if(t>=IMP)base=.25*Math.exp(-(t-IMP)/.5);
      setS(edge,'opacity',Math.min(1,base+.68*I).toFixed(3));
      var bo=eOut((t-.25)/.3),bf=eIn((t-2.0)/.28),bw=bo*(1-bf);
      setS(ban,'opacity',bw>0?'1':'0');setS(ban,'clipPath','inset(0 '+(50*(1-bw)).toFixed(2)+'% 0 '+(50*(1-bw)).toFixed(2)+'%)');
      setS(bt,'letterSpacing',lerp(.3,.08,eOut((t-.3)/.45)).toFixed(3)+'em');setT(bn,String(Math.max(n,1)));
      var slot=3.05*(eio((t-2.12)/.24)-eio((t-6.0)/.24));
      chip(mg,eOut(t/.3)*(1-eIn((t-7.4)/.2)),slot);
      chip(thr,eOut((t-2.12)/.3)*(1-eIn((t-5.8)/.2)),0);
      if(n>0){if(ths._m!==n){ths._m=n;ths.innerHTML='Impact in <b>'+n+'</b>';}}else if(ths._m!==0){ths._m=0;ths.innerHTML='<b>Impact</b> • stay down';}
      var rem=30*(1-c01(t/7.4));setS(ring,'strokeDashoffset',(100-100*rem/30).toFixed(2));setT(rv,String(Math.ceil(rem)));
      var ip=t-IMP;setS(imp,'opacity',ip>=0?(.2*Math.exp(-ip/.12)).toFixed(3):'0');
      if(ip>=0&&ip<.45){var amp=.42*(1-ip/.45);setS(map,'translate',(Math.sin(ip*90)*amp).toFixed(3)+'cqw '+(Math.cos(ip*70)*amp*.6).toFixed(3)+'cqw');}else setS(map,'translate','0 0');
    };
  });

  /* ================================================= 08 MATCH END */
  add('#fr-end',function(s){
    var el=s.el,P=24;s.still=9.5;
    var cv=$(el,'canvas.bg'),ctx=cv.getContext('2d'),svg=$(el,'.climb svg'),GR=$(svg,'.gr'),OT=$(svg,'.ot'),SE=$(svg,'.se'),cn=$(el,'.climb .cn'),cs=$(el,'.climb .cs');
    var rowsB=$$(el,'.trow'),cards=$$(el,'.mc'),c4=$(el,'.cards4'),nxt=$(el,'.nxt'),hint=$(el,'.vote .hint'),tb=new TickBar($(el,'.vote canvas.tickbar')),pass=Pass(el);
    tickbars.push(tb);
    var CL=DATA.climbs,END=DATA.end,X0=22,X1=462,pinned=0,shown=-1,userVote=null;
    function xt(v){return X0+(X1-X0)*v/600}function yr(r){return 134-124*(r-1)/13}
    function ns(tag,attrs){var e=document.createElementNS('http://www.w3.org/2000/svg',tag);for(var k in attrs)e.setAttribute(k,attrs[k]);return e;}
    function mmss(v){v=Math.round(v);return Math.floor(v/60)+':'+pad2(v%60)}
    /* grid */
    for(var m=0;m<=10;m+=2){var x=xt(m*60);GR.appendChild(ns('line',{class:'grid',x1:x,y1:6,x2:x,y2:160}));var tx=ns('text',{x:x,y:171,'text-anchor':'middle'});tx.textContent=m+':00';GR.appendChild(tx);}
    [1,7,14].forEach(function(r){GR.appendChild(ns('line',{class:'grid',x1:X0,y1:yr(r),x2:X1,y2:yr(r)}));var tx=ns('text',{x:16,y:yr(r)+2.2,'text-anchor':'end'});tx.textContent=pad2(r);GR.appendChild(tx);});
    GR.appendChild(ns('line',{class:'ax',x1:X0,y1:140.5,x2:X1,y2:140.5}));
    [['K',147],['D',157]].forEach(function(q){var tx=ns('text',{x:16,y:q[1]+2.2,'text-anchor':'end'});tx.textContent=q[0];GR.appendChild(tx);});
    function stepD(p){var d='M'+X0+','+yr(1);p.up.forEach(function(u,i){d+=' H'+xt(u).toFixed(1)+' V'+yr(i+2).toFixed(1);});return d+' H'+xt(END).toFixed(1);}
    var others=CL.map(function(p){var pa=ns('path',{class:'oth',d:stepD(p)});OT.appendChild(pa);return pa;});
    function render(i,anim){
      if(i===shown)return;shown=i;var p=CL[i];SE.innerHTML='';
      others.forEach(function(o,j){o.style.opacity=j===i?'0':'1'});
      var times=[0].concat(p.up,[END]),bi=0,bg=0;for(var k=0;k<times.length-1;k++){var g=times[k+1]-times[k];if(g>bg){bg=g;bi=k;}}
      var st=ns('g',{class:'st'});st.appendChild(ns('rect',{x:xt(times[bi]),y:yr(bi+1)-5,width:xt(times[bi+1])-xt(times[bi]),height:10}));
      var lt=ns('text',{x:(xt(times[bi])+xt(times[bi+1]))/2,y:yr(bi+1)-8,'text-anchor':'middle'});lt.textContent='Stall '+mmss(bg)+' • R'+pad2(bi+1);st.appendChild(lt);SE.appendChild(st);
      var path=ns('path',{class:'sel'+(p.w?' w':''),d:stepD(p),pathLength:1});SE.appendChild(path);
      var ru=ns('g',{class:'ru'+(p.w?' w':'')});p.up.forEach(function(u,k){var r=ns('rect',{x:xt(u)-1.6,y:yr(k+2)-1.6,width:3.2,height:3.2});r._x=xt(u);ru.appendChild(r);});SE.appendChild(ru);
      var kl=ns('g',{class:'kl'});p.kills.forEach(function(u){var r=ns('rect',{x:xt(u)-.6,y:144,width:1.2,height:6});r._x=xt(u);kl.appendChild(r);});SE.appendChild(kl);
      var dl=ns('g',{class:'dl'});p.deaths.forEach(function(u){var r=ns('rect',{x:xt(u)-.6,y:154,width:1.2,height:6});r._x=xt(u);dl.appendChild(r);});SE.appendChild(dl);
      var fin=ns('text',{class:p.w?'a':'w',x:xt(END),y:yr(p.r)-5,'text-anchor':'end'});fin.textContent=p.w?'14 • Knife':'R'+pad2(p.r);SE.appendChild(fin);
      cn.textContent=p.n;
      cs.textContent=(p.w?'All 14 rungs in '+mmss(END):'Reached rung '+pad2(p.r))+' • '+p.k+' K • '+p.d+' D';
      if(anim&&!RM&&path.animate){
        path.style.strokeDasharray='1';
        path.animate([{strokeDashoffset:1},{strokeDashoffset:0}],{duration:700,easing:'cubic-bezier(.16,1,.3,1)',fill:'both'});
        [ru,kl,dl].forEach(function(g,gi){[].forEach.call(g.children,function(r){r.animate([{opacity:0},{opacity:1}],{duration:180,delay:80+gi*60+(r._x-X0)/(X1-X0)*520,fill:'both'});});});
        [st,fin].forEach(function(g){g.animate([{opacity:0},{opacity:1}],{duration:260,delay:520,fill:'both'});});
      }
    }
    function sel(i,anim){render(i==null?pinned:i,anim);}
    rowsB.forEach(function(b,i){
      b.addEventListener('mouseenter',function(){sel(i,true)});b.addEventListener('focus',function(){sel(i,true)});
      b.addEventListener('mouseleave',function(){sel(null,true)});b.addEventListener('blur',function(){sel(null,true)});
      b.addEventListener('click',function(){pinned=i;rowsB.forEach(function(x,j){x.setAttribute('aria-pressed',j===i?'true':'false')});sel(i,true);});
    });
    render(0,false);
    /* vote */
    var VOTES=[[3.6,0],[4.4,2],[5.1,0],[6.3,1],[7.9,0],[9.4,3],[11.2,2]],MYT=5.6,MYDEF=2;
    radios(null,cards,function(b,i){if(c4.classList.contains('locked'))return;userVote=i;s.update(s.t);});
    var lastV='';
    /* background, one rule: broad slanted planes of width w in {16,32,64,128} units (1 unit = 1/192 of the width); each width
       repeats every 3 w, so each layer covers a third and the layers overlap, their 2% fills stacking into calm bands; each
       layer moves one spacing per 96 s, so speed is tied to width (the widest drift 4 units a second). */
    var WS=[16,32,64,128],BGA=.02,BGP=96,BGS=3,EBG='both',EAMB=Ambient(el,{col:-1,bloom:[.64,.68],menu:false});
    function drawBg(T){
      if(EBG!=='planes'&&!el.classList.contains('mapmode'))EAMB.draw(T);
      if(el.classList.contains('mapmode')||RM&&cv._drawn===PALKEY+EBG)return;
      var d=Math.min(2,DPR()),W=cv.clientWidth,H=cv.clientHeight;if(!W)return;
      if(cv.width!==Math.round(W*d)){cv.width=Math.round(W*d);cv.height=Math.round(H*d);}
      var w=cv.width,h=cv.height,u=w/192,sk=h*TAN,tb=mod(T,BGP);ctx.clearRect(0,0,w,h);
      ctx.fillStyle=rgba(PAL.fg,EBG==='both'?.006:BGA);if(EBG==='amb'){cv._drawn=PALKEY+EBG;return;}
      WS.forEach(function(wu,k){
        var bw=Math.round(wu*u),sp=BGS*wu*u,off=mod(sp*k/4-sp*tb/BGP,sp);
        for(var x=off-sp*(1+Math.ceil(sk/sp));x<w+sp;x+=sp){var x0=Math.round(x);ctx.beginPath();ctx.moveTo(x0+sk,0);ctx.lineTo(x0+sk+bw,0);ctx.lineTo(x0+bw,h);ctx.lineTo(x0,h);ctx.closePath();ctx.fill();}
      });
      cv._drawn=PALKEY+EBG;
    }
    s.update=function(T){
      var t=mod(T,P);drawBg(T);
      var cnt=[0,0,0,0],mine=userVote!=null?userVote:(t>=MYT?MYDEF:null);
      VOTES.forEach(function(v){if(t>=v[0])cnt[v[1]]++});if(mine!=null)cnt[mine]++;
      var lead=-1,best=0;cnt.forEach(function(c,i){if(c>best){best=c;lead=i;}});
      var locked=t>=22.4,key=cnt.join()+'|'+mine+'|'+locked+'|'+lead;
      if(key!==lastV){lastV=key;
        cards.forEach(function(c,i){c.classList.toggle('mine',i===mine);c.setAttribute('aria-checked',i===mine?'true':'false');c.classList.toggle('lead',i===lead);c.classList.toggle('won',locked&&i===lead);
          $(c,'.n').textContent=cnt[i];$$(c,'.pips i').forEach(function(p,k){p.className=k<cnt[i]?((i===mine&&k===cnt[i]-1)?'me':'on'):''});});
        c4.classList.toggle('locked',locked);
        hint.textContent=locked?'Next: '+DATA.maps[lead].toUpperCase()+' • starting':'Vote closes and the next match starts at 00:00';
      }
      var left=t<2.4?20:Math.max(0,Math.ceil(22.4-t));setT(nxt,'00:'+pad2(left));
      var n=tb.n;tb.draw(Math.floor(c01((t-2.4)/20)*n),Math.floor(17/20*n));
      pass.set(RM?-1:mod(t-23.4,P)/1.2);
    };
    s.onPal=function(){cv._drawn='';EAMB.redraw();};
    var ebB=$$(document,'.ctl.ebg');ebB.forEach(function(b){b.tabIndex=b.getAttribute('aria-checked')==='true'?0:-1;});
    function setEbg(m){EBG=m;EAMB.set(m==='planes'?'off':(EAMB.glOK()?'gl':'2d'));cv._drawn='';ebB.forEach(function(x){var on=x.getAttribute('data-m')===m;x.setAttribute('aria-checked',on?'true':'false');x.tabIndex=on?0:-1;});s.update(RM?s.still:s.t);}
    radios(null,ebB,function(b){setEbg(b.getAttribute('data-m'));});
    var bgt=document.querySelector('.bgt');
    if(bgt)bgt.addEventListener('click',function(){var on=!el.classList.contains('mapmode');el.classList.toggle('mapmode',on);bgt.setAttribute('aria-pressed',on?'true':'false');if(!on){cv._drawn='';}if(RM)s.update(s.still);});
  });

  /* ------------------------------------------------ K and D: measure each header mark against its column's numbers */
  function textRight(el){var r=document.createRange();r.selectNodeContents(el);var b=r.getBoundingClientRect();return b.right;}
  function measureKD(){
    var out=[],T=[['Scoreboard','#fr-score .sbt'],['HUD board','#fr-hud .c2.ch'],['Standings','#fr-end .std']];
    T.forEach(function(q){
      var tb=$(document,q[1]);if(!tb)return;var kh=$$(tb,'.kh .kdi');if(kh.length<2)return;
      var res=[0,1].map(function(c){var hr=kh[c].getBoundingClientRect().right,mx=0,n=0;
        $$(tb,(c?'.kd.d':'.kd:not(.d)')).forEach(function(k){var dv=Math.abs(textRight(k)-hr);if(dv>mx)mx=dv;n++;});return {mx:mx,n:n};});
      out.push({t:q[0],k:res[0],d:res[1]});
    });
    var m=$(document,'.kdm');
    if(m)m.innerHTML='Measured live, header mark right edge against number right edge, worst row: '+out.map(function(o){return '<b>'+o.t+'</b> K '+o.k.mx.toFixed(2)+' px, D '+o.d.mx.toFixed(2)+' px ('+o.k.n+' rows)';}).join(' • ');
    return out;
  }

  /* ------------------------------------------------ run loop, visibility, replay */
  function startFrame(s){s.t=0;s.el.classList.remove('in');void s.el.offsetWidth;s.el.classList.add('in');if(s.onStart)s.onStart();s.update(0);setTimeout(relayoutBars,1300);}
  if('IntersectionObserver' in window){
    var io=new IntersectionObserver(function(es){
      es.forEach(function(e){
        var s=e.target._s;if(!s)return;
        s.vis=e.isIntersecting;e.target.classList.toggle('off',!e.isIntersecting);
        if(!RM&&e.isIntersecting&&e.intersectionRatio>=.2&&!e.target.classList.contains('in'))startFrame(s);
      });
    },{threshold:[0,.2,.5]});
    $$(document,'.frame').forEach(function(f){if(!f._s){f._s={el:f,t:0,vis:false,update:function(){},still:0};scenes.push(f._s);}io.observe(f);});
  }else{scenes.forEach(function(s){s.vis=true;s.el.classList.add('in')});}
  $$(document,'.replay').forEach(function(b){b.addEventListener('click',function(){var f=document.getElementById(b.getAttribute('aria-controls'));if(f&&f._s&&!RM)startFrame(f._s);});});
  function stills(){scenes.forEach(function(s){s.t=s.still;s.update(s.still)});}
  var cur0=root.getAttribute('data-pal')||'cobalt';
  setPal(cur0,false);
  if('ResizeObserver' in window){var rz=new ResizeObserver(function(){relayoutBars();pxProof();measureKD();if(RM)stills();});$$(document,'.frame').forEach(function(f){rz.observe(f)});}
  else window.addEventListener('resize',function(){relayoutBars();pxProof();});
  function fontsReady(){setTimeout(measureKD,50);namesPanel();scenes.forEach(function(s){if(s.onFonts)s.onFonts();});relayoutBars();pxProof();if(RM)stills();}
  if(document.fonts&&document.fonts.load){Promise.all([document.fonts.load('700 20px "UGG Display"'),document.fonts.load('400 20px "UGG Display"'),document.fonts.load('20px "UGG Mono"')]).then(fontsReady,fontsReady);}
  else fontsReady();
  relayoutBars();pxProof();
  if(RM){stills();}
  else{
    var last=performance.now();
    var loop=function(now){
      var dt=Math.min(.1,(now-last)/1000);last=now;
      for(var i=0;i<scenes.length;i++){var s=scenes[i];if(s.vis&&s.el.classList.contains('in')){s.t+=dt;s.update(s.t);}}
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }
  if(mq.addEventListener)mq.addEventListener('change',function(){location.reload()});
  window.__ugg={ladder:function(){return LADDER_API},show:function(){return SHOW_API},showMode:function(m){SHOWMODE=m},scenes:scenes,fitName:fitName,setPal:setPal,measureKD:measureKD};
})();
