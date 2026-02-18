using System;
using System.Runtime.InteropServices;
using static SuperDuper.NativeMethods.SuperDuperEngine;

/// <summary>
/// Minimal C# console test harness for the super_duper_ffi native library.
/// Tests: create engine → set scan paths → start scan → query groups → print results → destroy.
/// </summary>
class Program
{
    static void Main(string[] args)
    {
        Console.WriteLine("=== Super Duper FFI Test Harness ===\n");

        // 1. Create engine
        Console.Write("Creating engine... ");
        var handle = sd_engine_create("super_duper.db");
        if (handle == 0)
        {
            Console.WriteLine($"FAILED: {GetLastError()}");
            return;
        }
        Console.WriteLine($"OK (handle={handle})");

        // 2. Set scan paths
        var paths = args.Length > 0 ? args : new[] { @"../test-data/folder1", @"../test-data/folder2" };
        Console.Write($"Setting scan paths ({string.Join(", ", paths)})... ");
        var (pathPtrs, pathHandles) = MarshalUtf8StringArray(paths);
        var result = sd_engine_set_scan_paths(handle, pathPtrs, (uint)pathPtrs.Length);
        FreeUtf8StringArray(pathHandles);
        if (result != SdResultCode.Ok)
        {
            Console.WriteLine($"FAILED: {result} - {GetLastError()}");
            sd_engine_destroy(handle);
            return;
        }
        Console.WriteLine("OK");

        // 3. Start scan
        Console.Write("Starting scan... ");
        result = sd_scan_start(handle);
        if (result != SdResultCode.Ok)
        {
            Console.WriteLine($"FAILED: {result} - {GetLastError()}");
            sd_engine_destroy(handle);
            return;
        }
        Console.WriteLine("OK");

        // 4. Query duplicate groups
        Console.Write("\nQuerying duplicate groups... ");
        result = sd_query_duplicate_groups(handle, 0, 100, out var page);
        if (result != SdResultCode.Ok)
        {
            Console.WriteLine($"FAILED: {result} - {GetLastError()}");
            sd_engine_destroy(handle);
            return;
        }
        Console.WriteLine($"OK ({page.Count} groups, {page.TotalAvailable} total)\n");

        // 5. Print groups and their files
        for (int i = 0; i < page.Count; i++)
        {
            var groupPtr = page.Groups + i * Marshal.SizeOf<SdDuplicateGroup>();
            var group = Marshal.PtrToStructure<SdDuplicateGroup>(groupPtr);

            Console.WriteLine($"Group {group.Id}: {group.FileCount} files, " +
                              $"{group.FileSize} bytes each, {group.WastedBytes} bytes wasted");

            // Query files in this group
            result = sd_query_files_in_group(handle, group.Id, out var filePage);
            if (result == SdResultCode.Ok)
            {
                for (int j = 0; j < filePage.Count; j++)
                {
                    var filePtr = filePage.Files + j * Marshal.SizeOf<SdFileRecord>();
                    var file = Marshal.PtrToStructure<SdFileRecord>(filePtr);
                    var path = Marshal.PtrToStringUTF8(file.CanonicalPath);
                    Console.WriteLine($"  - {path}");
                }
                sd_free_file_record_page(ref filePage);
            }
            Console.WriteLine();
        }

        unsafe
        {
            sd_free_duplicate_group_page(ref page);
        }

        // 6. Destroy engine
        Console.Write("Destroying engine... ");
        result = sd_engine_destroy(handle);
        Console.WriteLine(result == SdResultCode.Ok ? "OK" : $"FAILED: {result}");

        Console.WriteLine("\n=== Test complete ===");
    }
}
