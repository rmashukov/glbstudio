struct Vertex {
    float3 position [[attribute(0)]];
    float2 texcoord [[attribute(1)]];
    float4 color0 [[attribute(2)]];
    float4 normal [[attribute(3)]];
};
struct Varying {
    float4 position [[position]];
    float2 uv;
    float4 color;
    float3 world_position;
    float3 world_normal;
};
struct Uniforms {
    float4x4 Model;
    float4x4 Projection;
    float4 _Time;
    float4 EyeExposure;
    float4 BaseColor;
    float4 Surface;
    float4 Emission;
    float4 Options;
};
vertex Varying vertexShader(Vertex v [[stage_in]], constant Uniforms& u [[buffer(0)]]) {
    Varying o;
    o.position = u.Projection * u.Model * float4(v.position, 1.0);
    o.position.z = (o.position.z + o.position.w) * 0.5;
    o.uv = v.texcoord;
    o.color = v.color0 / 255.0;
    o.world_position = v.position;
    o.world_normal = v.normal.xyz;
    return o;
}
fragment float4 fragmentShader(Varying i [[stage_in]], bool front [[front_facing]], constant Uniforms& u [[buffer(0)]],
    texture2d<float> Texture [[texture(0)]], sampler s0 [[sampler(0)]],
    texture2d<float> MetallicRoughness [[texture(1)]], sampler s2 [[sampler(1)]],
    texture2d<float> NormalMap [[texture(2)]], sampler s3 [[sampler(2)]],
    texture2d<float> Occlusion [[texture(3)]], sampler s4 [[sampler(3)]],
    texture2d<float> EmissiveMap [[texture(4)]], sampler s5 [[sampler(4)]]) {
    float4 texel = Texture.sample(s0, i.uv);
    float alpha = texel.a * u.BaseColor.a * i.color.a;
    if (alpha < u.Surface.z) discard_fragment();
    float3 base = srgb_to_linear(texel.rgb) * u.BaseColor.rgb * i.color.rgb;
    float3 n = normalize(i.world_normal) * (front ? 1.0 : -1.0);
    if (u.Options.x > 0.5) {
        float3 dp1 = dfdx(i.world_position), dp2 = dfdy(i.world_position);
        float2 duv1 = dfdx(i.uv), duv2 = dfdy(i.uv);
        float3 t = cross(dp2, n) * duv1.x + cross(n, dp1) * duv2.x;
        float3 b = cross(dp2, n) * duv1.y + cross(n, dp1) * duv2.y;
        float inv = rsqrt(max(max(dot(t,t), dot(b,b)), 0.00000001));
        float3 mapped = NormalMap.sample(s3, i.uv).xyz * 2.0 - 1.0;
        mapped.xy *= u.Emission.w;
        n = normalize(t * inv * mapped.x + b * inv * mapped.y + n * mapped.z);
    }
    float4 mr = MetallicRoughness.sample(s2, i.uv);
    float ao = mix(1.0, Occlusion.sample(s4, i.uv).r, u.Options.y);
    float3 emission = srgb_to_linear(EmissiveMap.sample(s5, i.uv).rgb) * u.Emission.xyz;
    return float4(shade(base, n, normalize(u.EyeExposure.xyz - i.world_position), u.Surface.y * mr.g, u.Surface.x * mr.b, emission, ao, u.EyeExposure.w, u.Surface.w), u.Options.z > 0.5 ? 1.0 : alpha);
}
