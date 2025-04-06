static const float4 POINTS[3] = {
    float4(-1.0, -1.0, 0.0, 1.0),
    float4(1.0, -1.0, 0.0, 1.0),
    float4(0.0, 1.0, 0.0, 1.0)
};

[shader("vertex")]
float4 vert_main(uint id: SV_VertexID)
    : SV_Position
{
    return POINTS[id];
}

[shader("pixel")]
float4 frag_main()
    : SV_Target
{
    return float4(0.0, 1.0, 0.0, 1.0);
}
