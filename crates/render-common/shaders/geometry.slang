struct Vertex
{
    float2 position;
}

struct PushConstants
{
    Vertex *vertex_buffer;
}

[vk::push_constant]
uniform PushConstants pc;

[shader("vertex")]
float4 vert_main(uint id: SV_VertexID)
    : SV_Position
{
    return float4(pc.vertex_buffer[id].position, 0.0, 1.0);
}

[shader("pixel")]
float4 frag_main()
    : SV_Target
{
    return float4(0.0, 1.0, 0.0, 1.0);
}
