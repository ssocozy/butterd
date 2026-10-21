package dev.butterd.module;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.URL;
import javax.net.ssl.HttpsURLConnection;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONObject;

/** Metadata only. Uses Android's certificate-validated HTTPS stack; executes no downloaded code. */
public final class PixelCatalog {
    private static final String[] DEVICES = {"bluejay","panther","cheetah","lynx","shiba","husky","akita","tokay","caiman","komodo","tegu"};
    private static final String[] MODELS = {"Pixel 6a","Pixel 7","Pixel 7 Pro","Pixel 7a","Pixel 8","Pixel 8 Pro","Pixel 8a","Pixel 9","Pixel 9 Pro","Pixel 9 Pro XL","Pixel 9a"};
    private static final int[] INITIAL_API = {32,33,33,33,34,34,34,34,34,34,35};
    private static String securityPatch(String bulletin, String month) {
        Matcher rows = Pattern.compile("<tr\\b[^>]*>.*?</tr>", Pattern.DOTALL | Pattern.CASE_INSENSITIVE).matcher(bulletin);
        while (rows.find()) {
            String row = rows.group();
            if (!row.contains("/pixel/"+month.substring(0,4)+"/"+month+"-")) continue;
            Matcher cells = Pattern.compile("<td\\b[^>]*>(.*?)</td>", Pattern.DOTALL | Pattern.CASE_INSENSITIVE).matcher(row);
            String last = "";
            while (cells.find()) last = cells.group(1).replaceAll("<[^>]*>", "").trim();
            if (last.matches(Pattern.quote(month)+"-(?:0[1-9]|[12][0-9]|3[01])")) return last;
        }
        return null;
    }
    private static long deadline;
    private static String get(String address) throws Exception {
        if (System.currentTimeMillis() >= deadline) throw new IllegalStateException("Fetch deadline reached");
        HttpsURLConnection connection = (HttpsURLConnection) new URL(address).openConnection();
        connection.setConnectTimeout(4000); connection.setReadTimeout(4000);
        connection.setInstanceFollowRedirects(false);
        connection.setRequestProperty("Referer", "https://flash.android.com");
        try {
            if (connection.getResponseCode() != 200) throw new IllegalStateException("Metadata server unavailable");
            try (InputStream input = connection.getInputStream(); ByteArrayOutputStream output = new ByteArrayOutputStream()) {
                byte[] buffer = new byte[8192]; int count;
                while ((count = input.read(buffer)) >= 0) {
                    if (System.currentTimeMillis() >= deadline || output.size() + count > 4194304)
                        throw new IllegalStateException("Fetch budget exceeded");
                    output.write(buffer, 0, count);
                }
                return new String(output.toByteArray(), StandardCharsets.UTF_8);
            }
        } finally { connection.disconnect(); }
    }
    public static void main(String[] args) throws Exception {
        if (args.length != 1) throw new IllegalArgumentException("Expected profile directory");
        Path directory = java.nio.file.Paths.get(args[0]); Files.createDirectories(directory);
        deadline = System.currentTimeMillis() + 45000;
        Matcher key = Pattern.compile("AIza[0-9A-Za-z_-]{30,}").matcher(get("https://flash.android.com"));
        if (!key.find()) throw new IllegalStateException("Flash metadata key not found");
        String apiKey = key.group();
        String bulletin = "";
        try { bulletin = get("https://source.android.com/docs/security/bulletin/pixel"); } catch (Exception ignored) { }
        int updated = 0;
        for (int index = 0; index < DEVICES.length; index++) {
            try {
                String device = DEVICES[index];
                JSONArray builds = new JSONObject(get("https://content-flashstation-pa.googleapis.com/v1/builds?product="+device+"_beta&key="+apiKey)).getJSONArray("flashstationBuild");
                JSONObject latest = null; long latestId = -1;
                for (int i = 0; i < builds.length(); i++) {
                    JSONObject build = builds.getJSONObject(i), meta = build.optJSONObject("previewMetadata");
                    String id = build.optString("buildId");
                    if (meta != null && meta.optBoolean("canary") && build.optString("product").equals(device+"_beta") && id.matches("[0-9]{1,16}")) {
                        long number = Long.parseLong(id);
                        if (number > latestId) { latest = build; latestId = number; }
                    }
                }
                if (latest == null) continue;
                String release = latest.getString("releaseCandidateName");
                if (!release.matches("[A-Za-z0-9._-]{1,64}")) continue;
                String text = "FINGERPRINT=google/"+device+"_beta/"+device+":CANARY/"+release+"/"+latestId+":user/release-keys\nMANUFACTURER=Google\nMODEL="+MODELS[index]+"\n";
                Matcher period = Pattern.compile("canary-([0-9]{4})([0-9]{2})").matcher(latest.getJSONObject("previewMetadata").optString("id"));
                if (!period.matches()) throw new IllegalStateException("Unknown Canary month");
                String patch = securityPatch(bulletin, period.group(1)+"-"+period.group(2));
                if (patch == null) throw new IllegalStateException("No verified patch for Canary month");
                text += "SECURITY_PATCH="+patch+"\nDEVICE_INITIAL_SDK_INT="+INITIAL_API[index]+"\n";
                text += "spoofBuild=true\nspoofVendingBuild=true\nspoofProps=true\n";
                Path temporary = directory.resolve(device+".tmp");
                Files.write(temporary, text.getBytes(StandardCharsets.UTF_8));
                Files.move(temporary, directory.resolve(device+".prop"), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
                updated++;
            } catch (Exception unavailable) {
                System.out.println("Keeping bundled profile: "+MODELS[index]);
            }
        }
        System.out.println("Profiles refreshed: "+updated);
    }
}
