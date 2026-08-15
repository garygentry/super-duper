using System.Text.Json;
using System.Text.Json.Serialization;

namespace SuperDuper.Windows.Infrastructure.Protocol;

internal sealed class ResponseEnvelope
{
    [JsonPropertyName("type")]
    public string Type { get; init; } = string.Empty;

    [JsonPropertyName("id")]
    public string Id { get; init; } = string.Empty;

    [JsonPropertyName("ok")]
    public bool Ok { get; init; }

    [JsonPropertyName("result")]
    public JsonElement? Result { get; init; }

    [JsonPropertyName("error")]
    public ProtocolError? Error { get; init; }
}

internal sealed class ProtocolError
{
    [JsonPropertyName("code")]
    public string Code { get; init; } = string.Empty;

    [JsonPropertyName("message")]
    public string Message { get; init; } = string.Empty;

    [JsonPropertyName("retryable")]
    public bool Retryable { get; init; }

    [JsonPropertyName("details")]
    public JsonElement Details { get; init; }
}

internal sealed class EventEnvelope
{
    public required string Name { get; init; }

    public required JsonElement Data { get; init; }
}

internal readonly record struct InboundFrame(ResponseEnvelope? Response, EventEnvelope? Event);
