struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) rect_center: vec2<f32>,
    @location(2) rect_half_size: vec2<f32>,
    @location(3) corner_radius: f32,
    @location(4) shadow_size: f32,
    @location(5) shadow_color: vec4<f32>,
    @location(6) frag_pos: vec2<f32>,
};

struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) pos_px: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) rect_center: vec2<f32>,
    @location(4) rect_half_size: vec2<f32>,
    @location(5) corner_radius: f32,
    @location(6) shadow_size: f32,
    @location(7) shadow_color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(input.pos, 0.0, 1.0);
    out.color = input.color;
    out.rect_center = input.rect_center;
    out.rect_half_size = input.rect_half_size;
    out.corner_radius = input.corner_radius;
    out.shadow_size = input.shadow_size;
    out.shadow_color = input.shadow_color;
    // Pass pixel position to fragment shader for SDF calculation
    out.frag_pos = input.pos_px;
    return out;
}

fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2(radius);
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // If corner_radius and shadow_size are both 0, fast path: just return the color
    if input.corner_radius <= 0.0 && input.shadow_size <= 0.0 {
        return input.color;
    }

    // Compute fragment position relative to rect center (in clip space)
    let p = input.frag_pos - input.rect_center;
    let half_size = input.rect_half_size;
    let radius = input.corner_radius;

    let d = sdf_rounded_rect(p, half_size, radius);

    // Anti-aliased edge for the fill
    let fill_alpha = 1.0 - smoothstep(-1.0, 1.0, d);
    var final_color = input.color;
    final_color = vec4<f32>(final_color.rgb, final_color.a * fill_alpha);

    // Shadow (rendered outside the rect)
    if input.shadow_size > 0.0 {
        let sigma = input.shadow_size * 0.5;
        let shadow_alpha = exp(-max(d, 0.0) * max(d, 0.0) / (2.0 * sigma * sigma));
        let shadow = vec4<f32>(input.shadow_color.rgb, input.shadow_color.a * shadow_alpha * (1.0 - fill_alpha));
        
        let out_a = final_color.a + shadow.a * (1.0 - final_color.a);
        var out_rgb = final_color.rgb;
        if out_a > 0.0 {
            out_rgb = (final_color.rgb * final_color.a + shadow.rgb * shadow.a * (1.0 - final_color.a)) / out_a;
        }
        
        final_color = vec4<f32>(out_rgb, out_a);
    }

    return final_color;
}
