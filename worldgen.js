// Valheim biome generator: a port of the game's WorldGenerator.GetBiome / GetBaseHeight (1.0, gen version 2),
// checked against the placed locations of a real world (every boss altar, trader and dungeon lands in its biome).
// Unity's Mathf.PerlinNoise and Random (xorshift128) are rebuilt here; the game code calls them.
const f=Math.fround;
// Unity Mathf.PerlinNoise
const P=[151,160,137,91,90,15,131,13,201,95,96,53,194,233,7,225,140,36,103,30,69,142,8,99,37,240,21,10,23,190,6,148,247,120,234,75,0,26,197,62,94,252,219,203,117,35,11,32,57,177,33,88,237,149,56,87,174,20,125,136,171,168,68,175,74,165,71,134,139,48,27,166,77,146,158,231,83,111,229,122,60,211,133,230,220,105,92,41,55,46,245,40,244,102,143,54,65,25,63,161,1,216,80,73,209,76,132,187,208,89,18,169,200,196,135,130,116,188,159,86,164,100,109,198,173,186,3,64,52,217,226,250,124,123,5,202,38,147,118,126,255,82,85,212,207,206,59,227,47,16,58,17,182,189,28,42,223,183,170,213,119,248,152,2,44,154,163,70,221,153,101,155,167,43,172,9,129,22,39,253,19,98,108,110,79,113,224,232,178,185,112,104,218,246,97,228,251,34,242,193,238,210,144,12,191,179,162,241,81,51,145,235,249,14,239,107,49,192,214,31,181,199,106,157,184,84,204,176,115,121,50,45,127,4,150,254,138,236,205,93,222,114,67,29,24,72,243,141,128,195,78,66,215,61,156,180];
const p=new Uint8Array(512);for(let i=0;i<512;i++)p[i]=P[i&255];
const fade=t=>t*t*t*(t*(t*6-15)+10),lerp=(t,a,b)=>a+t*(b-a);
// Ken Perlin's improved-noise gradients at z = 0
const grad=(h,x,y)=>{h&=15;const u=h<8?x:y,v=h<4?y:(h===12||h===14)?x:0;return((h&1)===0?u:-u)+((h&2)===0?v:-v)};
function perlin(x,y){x=Math.abs(f(x));y=Math.abs(f(y));   // Unity mirrors negative coordinates
  const fx=Math.floor(x),fy=Math.floor(y),X=fx&255,Y=fy&255;x-=fx;y-=fy;
  const u=fade(x),v=fade(y),A=p[X]+Y,B=p[X+1]+Y;
  const q=i=>p[p[i]];   // 3D noise with z = 0: one more permutation step
  const r=lerp(v,lerp(u,grad(q(A),x,y),grad(q(B),x-1,y)),lerp(u,grad(q(A+1),x,y-1),grad(q(B+1),x-1,y-1)));
  return f((r+0.69)/1.483)}
// Unity Random (xorshift128)
function unityRandom(seed){let s0=seed>>>0,s1=(Math.imul(s0,1812433253)+1)>>>0,s2=(Math.imul(s1,1812433253)+1)>>>0,s3=(Math.imul(s2,1812433253)+1)>>>0;
  const next=()=>{let t=(s0^(s0<<11))>>>0;s0=s1;s1=s2;s2=s3;s3=(s3^(s3>>>19)^t^(t>>>8))>>>0;return s3};
  return{range:(a,b)=>a+(next()%((b-a)>>>0))}}
const clamp01=v=>v>1?1:v<0?0:v,lerpStep=(a,b,v)=>clamp01((v-a)/(b-a)),smoothStep=(a,b,v)=>{const t=clamp01((v-a)/(b-a));return t*t*(3-2*t)},L=(a,b,t)=>t<=0?a:t>=1?b:a*(1-t)+b*t;
export const BIOME={1:'Meadows',2:'Swamp',4:'Mountain',8:'Black Forest',16:'Plains',32:'Ashlands',64:'Deep North',256:'Ocean',512:'Mistlands'};
export function worldGen(seed,version=2){
  const R=unityRandom(seed),o0=R.range(-10000,10000),o1=R.range(-10000,10000),o2=R.range(-10000,10000);R.range(-10000,10000);
  R.range(-2147483648,2147483647);R.range(-2147483648,2147483647);const o4=R.range(-10000,10000);
  const minMtn=version<=0?1500:1000,minDark=version<=1?0.5:0.4,maxMarsh=version<=1?8000:6000;
  const angle=(x,y)=>f(Math.sin(f(Math.atan2(x,y))*20));
  function baseHeight(x,y){const d=f(Math.hypot(x,y));x+=100000+o0;y+=100000+o1;let h=0;
    h=f(h+perlin(x*0.002*0.5,y*0.002*0.5)*perlin(x*0.003*0.5,y*0.003*0.5));
    h=f(h+perlin(x*0.002,y*0.002)*perlin(x*0.003,y*0.003)*h*0.9);
    h=f(h+perlin(x*0.005,y*0.005)*perlin(x*0.01,y*0.01)*0.5*h);
    h=f(h-0.07);
    const a=perlin(x*0.002*0.25+0.123,y*0.002*0.25+0.15123),b=perlin(x*0.002*0.25+0.321,y*0.002*0.25+0.231);
    let g=f(1-lerpStep(0.02,0.12,Math.abs(a-b)));g=f(g*smoothStep(744,1000,d));h=f(h*(1-g));
    if(d>10000){h=L(h,-0.2,lerpStep(10000,10500,d));if(d>10490)h=L(h,-2,lerpStep(10490,10500,d));return h}
    if(d<minMtn&&h>0.28){const t=clamp01((h-0.28)/0.1);h=L(L(0.28,0.38,t),h,lerpStep(minMtn-400,minMtn,d))}
    return h}
  function biome(x,y){const d=f(Math.hypot(x,y)),h=baseHeight(x,y),an=f(angle(x,y)*100);
    if(f(Math.hypot(x,y-4000))>12000+an)return 32;
    if(h<=0.02)return 256;
    if(f(Math.hypot(x,y+4000))>12000+an)return 64;
    if(h>0.4)return 4;
    if(perlin((o0+x)*0.001,(o0+y)*0.001)>0.6&&d>2000&&d<maxMarsh&&h>0.05&&h<0.25)return 2;
    if(perlin((o4+x)*0.001,(o4+y)*0.001)>minDark&&d>6000+an&&d<10000)return 512;
    if(perlin((o1+x)*0.001,(o1+y)*0.001)>0.4&&d>3000+an&&d<8000)return 16;
    if(perlin((o2+x)*0.001,(o2+y)*0.001)>0.4&&d>600+an&&d<6000)return 8;
    if(d>5000+an)return 8;
    return 1}
  return{biome,baseHeight}}
