package app.omochikaeri.hagane;

import android.app.Activity;
import android.app.Fragment;
import android.app.FragmentManager;
import android.content.ClipData;
import android.content.ContentResolver;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

public class Glue extends Fragment {
    private static final String TAG = "app.omochikaeri.hagane.glue";

    private static final Uri DOWNLOADS_ROOT = DocumentsContract.buildRootUri("com.android.providers.downloads.documents", "downloads");

    private static native void picked(int request, String[] uris);

    private static Glue attach(Activity activity) {
        FragmentManager manager = activity.getFragmentManager();
        Glue glue = (Glue) manager.findFragmentByTag(TAG);
        if (glue == null) {
            glue = new Glue();
            manager.beginTransaction().add(glue, TAG).commitNow();
        }
        return glue;
    }

    public static void pick(final Activity activity, final boolean multiple, final int request) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("*/*");
                intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, DOWNLOADS_ROOT);
                if (multiple) {
                    intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
                }
                attach(activity).startActivityForResult(intent, request);
            }
        });
    }

    public static void save(final Activity activity, final String name, final String mime, final int request) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType(mime);
                intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, DOWNLOADS_ROOT);
                intent.putExtra(Intent.EXTRA_TITLE, name);
                attach(activity).startActivityForResult(intent, request);
            }
        });
    }

    public static void immersive(final Activity activity) {
        activity.runOnUiThread(new Runnable() {
            @Override
            public void run() {
                Window window = activity.getWindow();
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    window.setDecorFitsSystemWindows(false);
                    WindowInsetsController controller = window.getInsetsController();
                    if (controller != null) {
                        controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                        controller.hide(WindowInsets.Type.systemBars());
                    }
                } else {
                    window.getDecorView().setSystemUiVisibility(
                        View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                            | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_FULLSCREEN);
                }
            }
        });
    }

    @Override
    public void onActivityResult(int request, int result, Intent data) {
        String[] uris = null;
        if (result == Activity.RESULT_OK && data != null) {
            ClipData clip = data.getClipData();
            if (clip != null) {
                uris = new String[clip.getItemCount()];
                for (int i = 0; i < uris.length; i++) {
                    uris[i] = clip.getItemAt(i).getUri().toString();
                }
            } else if (data.getData() != null) {
                uris = new String[] { data.getData().toString() };
            }
        }
        picked(request, uris);
    }

    public static String copy(Activity activity, String uri, String folder) throws IOException {
        ContentResolver resolver = activity.getContentResolver();
        Uri parsed = Uri.parse(uri);
        String name = null;
        try {
            Cursor cursor = resolver.query(parsed, new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null);
            if (cursor != null) {
                try {
                    if (cursor.moveToFirst()) {
                        name = cursor.getString(0);
                    }
                } finally {
                    cursor.close();
                }
            }
        } catch (RuntimeException unnamed) {
            name = null;
        }
        if (name == null || name.isEmpty()) {
            String segment = parsed.getLastPathSegment();
            if (segment != null) {
                int cut = Math.max(segment.lastIndexOf('/'), segment.lastIndexOf(':'));
                name = cut >= 0 ? segment.substring(cut + 1) : segment;
            }
        }
        if (name == null || name.isEmpty()) {
            name = "picked";
        }
        File target = new File(folder, name.replace('/', '_'));
        File parent = target.getParentFile();
        if (parent != null) {
            parent.mkdirs();
        }
        InputStream in = open(resolver, parsed);
        try (OutputStream out = new FileOutputStream(target)) {
            pump(in, out);
        } finally {
            in.close();
        }
        return target.getAbsolutePath();
    }

    private static InputStream open(ContentResolver resolver, Uri parsed) throws IOException {
        String failure;
        try {
            InputStream in = resolver.openInputStream(parsed);
            if (in != null) {
                return in;
            }
            failure = "the provider returned no stream";
        } catch (RuntimeException | IOException streamless) {
            failure = "the provider failed to open it: " + streamless;
        }
        File direct = direct(parsed);
        if (direct != null) {
            try {
                return new FileInputStream(direct);
            } catch (IOException unreadable) {
                failure = failure + "; the path " + direct + " could not be read: " + unreadable;
            }
        }
        throw new IOException(parsed + ": " + failure);
    }

    private static File direct(Uri parsed) {
        String authority = parsed.getAuthority();
        String id;
        try {
            id = DocumentsContract.getDocumentId(parsed);
        } catch (RuntimeException undocumented) {
            return null;
        }
        if (authority == null || id == null) {
            return null;
        }
        if (authority.equals("com.android.externalstorage.documents") && id.startsWith("primary:")) {
            return new File(Environment.getExternalStorageDirectory(), id.substring("primary:".length()));
        }
        if (authority.equals("com.android.providers.downloads.documents") && id.startsWith("raw:")) {
            return new File(id.substring("raw:".length()));
        }
        return null;
    }

    public static void write(Activity activity, String uri, String path) throws IOException {
        ContentResolver resolver = activity.getContentResolver();
        OutputStream out = resolver.openOutputStream(Uri.parse(uri), "wt");
        if (out == null) {
            throw new IOException("the document could not be opened for writing");
        }
        try (InputStream in = new FileInputStream(path)) {
            pump(in, out);
        } finally {
            out.close();
        }
    }

    private static void pump(InputStream in, OutputStream out) throws IOException {
        byte[] buffer = new byte[1 << 16];
        int read;
        while ((read = in.read(buffer)) > 0) {
            out.write(buffer, 0, read);
        }
    }
}
