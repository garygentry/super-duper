using System;
using System.Runtime.InteropServices;

namespace SuperDuper.NativeMethods;

/// <summary>
/// P/Invoke declarations for the super_duper_ffi native library.
/// </summary>
public static partial class SuperDuperEngine
{
    private const string DllName = "super_duper_ffi";

    // ── Result Codes ─────────────────────────────────────────────

    public enum SdResultCode : int
    {
        Ok = 0,
        InvalidHandle = 1,
        InvalidArgument = 2,
        IoError = 3,
        DatabaseError = 4,
        ScanInProgress = 5,
        ScanNotRunning = 6,
        InternalError = 99,
    }

    // ── Structs ──────────────────────────────────────────────────

    [StructLayout(LayoutKind.Sequential)]
    public struct SdDuplicateGroup
    {
        public long Id;
        public long ContentHash;
        public long FileSize;
        public long FileCount;
        public long WastedBytes;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdDuplicateGroupPage
    {
        public IntPtr Groups;
        public uint Count;
        public uint TotalAvailable;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdFileRecord
    {
        public long Id;
        public IntPtr CanonicalPath;
        public IntPtr FileName;
        public IntPtr ParentDir;
        public long FileSize;
        public long ContentHash;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdFileRecordPage
    {
        public IntPtr Files;
        public uint Count;
    }

    // ── Callbacks ────────────────────────────────────────────────

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    public delegate void SdProgressCallback(
        uint phase,        // 0=scan, 1=hash, 2=db_write
        ulong current,
        ulong total,
        IntPtr message     // const char*
    );

    // ── Engine Lifecycle ─────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern ulong sd_engine_create(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string dbPath);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_engine_destroy(ulong handle);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_engine_set_scan_paths(
        ulong handle,
        [MarshalAs(UnmanagedType.LPArray, ArraySubType = UnmanagedType.LPUTF8Str)]
        string[] paths,
        uint count);

    // ── Scan Operations ──────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_scan_start(ulong handle);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool sd_scan_is_running(ulong handle);

    // ── Queries ──────────────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_query_duplicate_groups(
        ulong handle,
        long offset,
        long limit,
        out SdDuplicateGroupPage page);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_query_files_in_group(
        ulong handle,
        long groupId,
        out SdFileRecordPage page);

    // ── Memory Management ────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void sd_free_duplicate_group_page(ref SdDuplicateGroupPage page);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void sd_free_file_record_page(ref SdFileRecordPage page);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void sd_free_string(IntPtr ptr);

    // ── Error Handling ───────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr sd_last_error_message();

    // ── Deletion ─────────────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_mark_file_for_deletion(
        ulong handle, long fileId);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_unmark_file_for_deletion(
        ulong handle, long fileId);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_deletion_plan_summary(
        ulong handle, out long count, out long bytes);

    // ── Helpers ──────────────────────────────────────────────────

    public static string? GetLastError()
    {
        var ptr = sd_last_error_message();
        if (ptr == IntPtr.Zero) return null;
        var msg = Marshal.PtrToStringUTF8(ptr);
        sd_free_string(ptr);
        return msg;
    }

    public static string? PtrToStringAndFree(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return null;
        var str = Marshal.PtrToStringUTF8(ptr);
        sd_free_string(ptr);
        return str;
    }
}
