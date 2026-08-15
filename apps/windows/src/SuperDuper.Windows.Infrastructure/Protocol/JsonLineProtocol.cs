using System.Text;
using System.Text.Json;

namespace SuperDuper.Windows.Infrastructure.Protocol;

internal static class JsonLineProtocol
{
    public const int MaximumFrameBytes = 1_048_576;

    internal static readonly JsonSerializerOptions SerializerOptions = new(JsonSerializerDefaults.Web);

    public static string EncodeRequestFrame(string id, string method, object parameters)
    {
        ArgumentException.ThrowIfNullOrEmpty(id);
        ArgumentException.ThrowIfNullOrEmpty(method);
        ArgumentNullException.ThrowIfNull(parameters);

        var json = JsonSerializer.Serialize(
            new { type = "request", id, method, @params = parameters },
            SerializerOptions);

        if (Encoding.UTF8.GetByteCount(json) > MaximumFrameBytes)
        {
            throw new WorkerProtocolException($"Request frame exceeds {MaximumFrameBytes} bytes.");
        }

        return json + "\n";
    }

    public static InboundFrame ParseInboundFrame(string line)
    {
        if (string.IsNullOrEmpty(line))
        {
            throw new WorkerProtocolException("Worker wrote an empty protocol frame.");
        }

        if (Encoding.UTF8.GetByteCount(line) > MaximumFrameBytes)
        {
            throw new WorkerProtocolException($"Worker frame exceeds {MaximumFrameBytes} bytes.");
        }

        try
        {
            using var document = JsonDocument.Parse(line);
            var root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object ||
                !root.TryGetProperty("type", out var typeElement) ||
                typeElement.ValueKind != JsonValueKind.String)
            {
                throw new WorkerProtocolException("Worker frame is not a typed JSON object.");
            }

            return typeElement.GetString() switch
            {
                "response" => ParseResponse(line),
                "event" => ParseEvent(root),
                var type => throw new WorkerProtocolException($"Unknown worker frame type: {type}"),
            };
        }
        catch (JsonException exception)
        {
            throw new WorkerProtocolException("Worker wrote malformed JSON.", exception);
        }
    }

    private static InboundFrame ParseResponse(string line)
    {
        var response = JsonSerializer.Deserialize<ResponseEnvelope>(line, SerializerOptions)
            ?? throw new WorkerProtocolException("Worker response could not be read.");

        if (string.IsNullOrEmpty(response.Id))
        {
            throw new WorkerProtocolException("Worker response has no request ID.");
        }

        if (response.Ok && response.Result is null)
        {
            throw new WorkerProtocolException("Successful worker response has no result.");
        }

        if (!response.Ok && response.Error is null)
        {
            throw new WorkerProtocolException("Failed worker response has no structured error.");
        }

        return new InboundFrame(response, Event: null);
    }

    private static InboundFrame ParseEvent(JsonElement root)
    {
        if (!root.TryGetProperty("event", out var eventName) ||
            eventName.ValueKind != JsonValueKind.String ||
            !root.TryGetProperty("data", out var data) ||
            data.ValueKind != JsonValueKind.Object)
        {
            throw new WorkerProtocolException("Worker event envelope is invalid.");
        }

        return new InboundFrame(
            Response: null,
            Event: new EventEnvelope
            {
                Name = eventName.GetString()!,
                Data = data.Clone(),
            });
    }
}
