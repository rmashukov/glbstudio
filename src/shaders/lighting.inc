vec3 srgb_to_linear(vec3 c) {
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}
vec3 linear_to_srgb(vec3 c) {
    c = max(c, vec3(0.0));
    return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}
vec3 light_brdf(vec3 base, vec3 n, vec3 v, vec3 l, vec3 radiance, float rough, float metal) {
    vec3 h = normalize(v + l);
    float nl = max(dot(n, l), 0.0);
    float nv = max(dot(n, v), 0.001);
    float nh = max(dot(n, h), 0.0);
    float vh = max(dot(v, h), 0.0);
    float a = rough * rough;
    float a2 = a * a;
    float d = nh * nh * (a2 - 1.0) + 1.0;
    float distribution = a2 / max(3.14159265 * d * d, 0.00001);
    float k = (rough + 1.0) * (rough + 1.0) / 8.0;
    float visibility = nv / (nv * (1.0 - k) + k) * nl / (nl * (1.0 - k) + k);
    vec3 f0 = mix(vec3(0.04), base, metal);
    vec3 fresnel = f0 + (1.0 - f0) * pow(1.0 - vh, 5.0);
    vec3 specular = distribution * visibility * fresnel / max(4.0 * nl * nv, 0.001);
    vec3 diffuse = (1.0 - fresnel) * (1.0 - metal) * base / 3.14159265;
    return (diffuse + specular) * radiance * nl;
}
vec3 shade(vec3 base, vec3 n, vec3 v, float rough, float metal, vec3 emission, float ao, float exposure, float unlit) {
    if (unlit > 0.5) return linear_to_srgb(base);
    rough = clamp(rough, 0.08, 1.0);
    metal = clamp(metal, 0.0, 1.0);
    vec3 hemi = mix(vec3(0.14, 0.16, 0.20), vec3(0.46, 0.53, 0.62), n.y * 0.5 + 0.5);
    vec3 color = base * hemi * (1.0 - metal * 0.65) * ao;
    color += mix(vec3(0.04), base, metal) * hemi * 0.3 * ao;
    color += light_brdf(base, n, v, normalize(vec3(-3.0, 5.0, 4.0)), vec3(3.8, 3.5, 3.1), rough, metal);
    color += light_brdf(base, n, v, normalize(vec3(4.0, 2.0, 1.0)), vec3(1.0, 1.4, 2.0), rough, metal);
    color += light_brdf(base, n, v, normalize(vec3(1.0, 3.0, -4.0)), vec3(2.4, 2.9, 3.4), rough, metal);
    color = (color + emission) * exposure;
    color = clamp((color * (2.51 * color + 0.03)) / (color * (2.43 * color + 0.59) + 0.14), vec3(0.0), vec3(1.0));
    return linear_to_srgb(color);
}
