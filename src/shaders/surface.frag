varying vec2 uv;
varying vec4 color;
varying vec3 world_position;
varying vec3 world_normal;
uniform sampler2D Texture;
uniform sampler2D MetallicRoughness;
uniform sampler2D NormalMap;
uniform sampler2D Occlusion;
uniform sampler2D EmissiveMap;
uniform vec4 EyeExposure;
uniform vec4 BaseColor;
uniform vec4 Surface;
uniform vec4 Emission;
uniform vec4 Options;
void main() {
    vec4 texel = texture2D(Texture, uv);
    float alpha = texel.a * BaseColor.a * color.a;
    if (alpha < Surface.z) discard;
    vec3 base = srgb_to_linear(texel.rgb) * BaseColor.rgb * color.rgb;
    vec3 n = normalize(world_normal);
    if (!gl_FrontFacing) n = -n;
    if (Options.x > 0.5) {
        vec3 dp1 = dFdx(world_position), dp2 = dFdy(world_position);
        vec2 duv1 = dFdx(uv), duv2 = dFdy(uv);
        vec3 t = cross(dp2, n) * duv1.x + cross(n, dp1) * duv2.x;
        vec3 b = cross(dp2, n) * duv1.y + cross(n, dp1) * duv2.y;
        float inv = inversesqrt(max(max(dot(t,t), dot(b,b)), 0.00000001));
        vec3 mapped = texture2D(NormalMap, uv).xyz * 2.0 - 1.0;
        mapped.xy *= Emission.w;
        n = normalize(t * inv * mapped.x + b * inv * mapped.y + n * mapped.z);
    }
    vec4 mr = texture2D(MetallicRoughness, uv);
    float ao = mix(1.0, texture2D(Occlusion, uv).r, Options.y);
    vec3 emission = srgb_to_linear(texture2D(EmissiveMap, uv).rgb) * Emission.xyz;
    vec3 lit = shade(base, n, normalize(EyeExposure.xyz - world_position), Surface.y * mr.g, Surface.x * mr.b, emission, ao, EyeExposure.w, Surface.w);
    gl_FragColor = vec4(lit, Options.z > 0.5 ? 1.0 : alpha);
}
