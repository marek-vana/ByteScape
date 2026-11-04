struct Clock { time:f32, padding_a:f32, padding_b:f32, padding_c:f32 };
@group(0) @binding(0) var<uniform> clock:Clock;
struct VertexOutput { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32> };
@vertex fn vertex_main(@builtin(vertex_index) id:u32)->VertexOutput {
    let uv=vec2<f32>(f32((id<<1u)&2u),f32(id&2u));
    var out:VertexOutput;out.position=vec4<f32>(uv*2.0-1.0,0.0,1.0);out.uv=uv;return out;
}
@fragment fn fragment_main(in:VertexOutput)->@location(0) vec4<f32> {
    let q=in.uv-0.5;
    let glow=exp(-dot(q*vec2<f32>(1.0,1.3),q*vec2<f32>(1.0,1.3))*6.0);
    let coord=in.uv*vec2<f32>(34.0,22.0);
    let dist=abs(fract(coord-0.5)-0.5)/max(fwidth(coord),vec2<f32>(0.0001));
    let grid=1.0-min(min(dist.x,dist.y),1.0);
    let pixel=in.position.xy;
    let dot_dist=length(abs(fract(pixel/18.0)-0.5)*18.0);
    let dots=1.0-smoothstep(0.35,1.05,dot_dist);
    let etched=1.0-smoothstep(0.0,0.8,abs(fract((pixel.x+pixel.y)/64.0)-0.5)*64.0);
    let noise=fract(sin(dot(floor(pixel),vec2<f32>(12.9898,78.233)))*43758.5453);
    let scanline=select(0.0,0.0015,u32(pixel.y)%4u==0u);
    let sweep_pos=fract(clock.time*0.013);
    let sweep=exp(-pow((in.uv.x-sweep_pos)*28.0,2.0))*select(0.0,1.0,clock.time>0.0);
    let bg=vec3<f32>(0.016,0.027,0.043);
    let tint=vec3<f32>(0.015,0.05,0.065);
    return vec4<f32>(bg+tint*(glow*0.42+grid*0.18+dots*0.35+etched*0.08+sweep*0.12)+(noise-0.5)*0.002-scanline,1.0);
}
