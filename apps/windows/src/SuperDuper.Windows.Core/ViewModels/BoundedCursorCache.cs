namespace SuperDuper.Windows.Core.ViewModels;

internal sealed class BoundedCursorCache<TValue>(int capacity)
    where TValue : class
{
    private const string FirstPageKey = "\0";
    private readonly Dictionary<string, TValue> _values = [];
    private readonly LinkedList<string> _leastRecentlyUsed = [];

    public int Count => _values.Count;

    public void Clear()
    {
        _values.Clear();
        _leastRecentlyUsed.Clear();
    }

    public bool TryGet(string? cursor, out TValue value)
    {
        var key = Key(cursor);
        if (_values.TryGetValue(key, out value!))
        {
            Touch(key);
            return true;
        }
        return false;
    }

    public void Set(string? cursor, TValue value)
    {
        var key = Key(cursor);
        _values[key] = value;
        Touch(key);
        while (_values.Count > capacity)
        {
            var oldest = _leastRecentlyUsed.First!;
            _leastRecentlyUsed.RemoveFirst();
            _values.Remove(oldest.Value);
        }
    }

    private static string Key(string? cursor) => cursor ?? FirstPageKey;

    private void Touch(string key)
    {
        _leastRecentlyUsed.Remove(key);
        _leastRecentlyUsed.AddLast(key);
    }
}
