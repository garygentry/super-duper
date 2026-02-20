namespace SuperDuper.Services.Platform;

public interface IFilePickerService
{
    /// <summary>Opens a folder picker dialog. Returns the selected path or null if cancelled.</summary>
    Task<string?> PickFolderAsync();

    /// <summary>Opens a save file dialog. Returns the selected path or null if cancelled.</summary>
    Task<string?> PickSaveFileAsync(string suggestedName, string fileTypeDescription = "CSV Files", string fileTypeExtension = ".csv");
}
