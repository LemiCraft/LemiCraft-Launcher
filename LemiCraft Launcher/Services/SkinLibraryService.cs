using LemiCraft_Launcher.Models;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Net.Http.Json;

namespace LemiCraft_Launcher.Services
{
    public static class SkinLibraryService
    {
        private static readonly HttpClient _httpClient = new();
        private static readonly System.Text.Json.JsonSerializerOptions JsonOptions = new()
        {
            PropertyNameCaseInsensitive = true
        };

        private static string GetApiUrl(string endpoint)
        {
            var config = ConfigService.Load();
            return $"{config.ApiBaseUrl}/launcher/skins/{endpoint}";
        }

        public static async Task<List<SkinLibraryItem>> GetUserSkinsAsync(string username, bool forceRefresh = false, string? provider = null)
        {
            try
            {
                if (!forceRefresh)
                {
                    var cachedSkins = await SkinCacheService.GetCachedSkinsAsync(username);
                    if (cachedSkins != null)
                        return cachedSkins;
                }

                var endpoint = provider != null
                    ? $"user/{Uri.EscapeDataString(username)}?provider={Uri.EscapeDataString(provider)}"
                    : $"user/{Uri.EscapeDataString(username)}";

                var url = GetApiUrl(endpoint);
                Debug.WriteLine($"🔍 Fetching: {url}");

                var response = await _httpClient.GetAsync(url);
                response.EnsureSuccessStatusCode();

                var result = await response.Content.ReadFromJsonAsync<UserSkinsResponse>(JsonOptions);
                var skins = result?.Skins ?? new List<SkinLibraryItem>();

                await SkinCacheService.SaveSkinsToCache(username, skins);

                return skins;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"❌ Error: {ex.Message}");

                var cachedSkins = await SkinCacheService.GetCachedSkinsAsync(username);
                return cachedSkins ?? new List<SkinLibraryItem>();
            }
        }

        public static async Task<UploadSkinResponse?> UploadSkinAsync(
            string filePath,
            string name,
            string model,
            string username)
        {
            try
            {
                using var content = new MultipartFormDataContent();

                var fileBytes = await File.ReadAllBytesAsync(filePath);
                content.Add(new ByteArrayContent(fileBytes), "file", Path.GetFileName(filePath));
                content.Add(new StringContent(name), "name");
                content.Add(new StringContent(model), "model");
                content.Add(new StringContent(username), "username");

                Debug.WriteLine($"📤 Uploading: {name}");

                var response = await _httpClient.PostAsync(GetApiUrl("upload"), content);
                response.EnsureSuccessStatusCode();

                var result = await response.Content.ReadFromJsonAsync<UploadSkinResponse>(JsonOptions);

                SkinCacheService.InvalidateSkinsCache(username);

                return result;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"❌ Error: {ex.Message}");
                return null;
            }
        }

        public static async Task<bool> ApplySkinAsync(
            int skinId,
            string username,
            string? accessToken = null,
            string? provider = null,
            string? uuid = null)
        {
            try
            {
                var body = new
                {
                    skinId,
                    username,
                    uuid,
                    provider,
                    accessToken
                };

                Debug.WriteLine($"🎨 Applying skin {skinId}");

                var response = await _httpClient.PostAsJsonAsync(GetApiUrl("apply"), body);
                response.EnsureSuccessStatusCode();

                var result = await response.Content.ReadFromJsonAsync<ApplySkinResponse>(JsonOptions);
                var success = result?.Success ?? false;

                if (success)
                    SkinCacheService.InvalidateSkinsCache(username);

                return success;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"❌ Error: {ex.Message}");
                return false;
            }
        }

        public static async Task<bool> DeleteSkinAsync(int skinId, string username)
        {
            try
            {
                var body = new { skinId, username };

                Debug.WriteLine($"🗑️ Deleting skin {skinId}");

                var response = await _httpClient.PostAsJsonAsync(GetApiUrl("delete"), body);
                response.EnsureSuccessStatusCode();

                var result = await response.Content.ReadFromJsonAsync<DeleteSkinResponse>(JsonOptions);
                var success = result?.Success ?? false;

                if (success)
                    SkinCacheService.InvalidateSkinsCache(username);
                return success;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"❌ Error: {ex.Message}");
                return false;
            }
        }

        private class DeleteSkinResponse
        {
            public bool Success { get; set; }
        }
    }
}