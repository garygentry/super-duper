using System;
using System.Runtime.InteropServices;
using System.Text;

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
        Cancelled = 7,
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
        public byte IsMarkedForDeletion;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdFileRecordPage
    {
        public IntPtr Files;
        public uint Count;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdDirectoryNode
    {
        public long Id;
        public IntPtr Path;
        public IntPtr Name;
        public long ParentId;
        public long TotalSize;
        public long FileCount;
        public long Depth;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdDirectoryNodePage
    {
        public IntPtr Nodes;
        public uint Count;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdDirectorySimilarity
    {
        public long Id;
        public long DirAId;
        public long DirBId;
        public double SimilarityScore;
        public long SharedBytes;
        public IntPtr MatchType;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdDirectorySimilarityPage
    {
        public IntPtr Pairs;
        public uint Count;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SdDeletionResult
    {
        public uint SuccessCount;
        public uint ErrorCount;
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
        IntPtr[] paths,
        uint count);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_engine_set_ignore_patterns(
        ulong handle,
        IntPtr[] patterns,
        uint count);

    // ── Scan Operations ──────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_scan_start(ulong handle);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool sd_scan_is_running(ulong handle);

    // ── Scan Cancellation ───────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_scan_cancel(ulong handle);

    // ── Progress Callback ──────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_set_progress_callback(
        ulong handle, SdProgressCallback callback);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_clear_progress_callback(ulong handle);

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

    // ── Directory Queries ────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_query_directory_children(
        ulong handle,
        long parentId,
        long offset,
        long limit,
        out SdDirectoryNodePage page);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_query_similar_directories(
        ulong handle,
        double minScore,
        long offset,
        long limit,
        out SdDirectorySimilarityPage page);

    // ── Memory Management ────────────────────────────────────────

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void sd_free_duplicate_group_page(ref SdDuplicateGroupPage page);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void sd_free_file_record_page(ref SdFileRecordPage page);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void sd_free_directory_node_page(ref SdDirectoryNodePage page);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void sd_free_directory_similarity_page(ref SdDirectorySimilarityPage page);

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

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_mark_directory_for_deletion(
        ulong handle,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string directoryPath);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_auto_mark_for_deletion(ulong handle);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern SdResultCode sd_deletion_execute(
        ulong handle, out SdDeletionResult result);

    // ── Helpers ──────────────────────────────────────────────────

    public static (IntPtr[] Ptrs, GCHandle[] Handles) MarshalUtf8StringArray(string[] strings)
    {
        var ptrs = new IntPtr[strings.Length];
        var handles = new GCHandle[strings.Length];
        for (int i = 0; i < strings.Length; i++)
        {
            var bytes = Encoding.UTF8.GetBytes(strings[i] + "\0");
            handles[i] = GCHandle.Alloc(bytes, GCHandleType.Pinned);
            ptrs[i] = handles[i].AddrOfPinnedObject();
        }
        return (ptrs, handles);
    }

    public static void FreeUtf8StringArray(GCHandle[] handles)
    {
        foreach (var h in handles) h.Free();
    }

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
