package dev.butterd.module;

import android.app.ActivityManager;
import android.app.admin.DevicePolicyManager;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.os.Looper;
import android.os.SystemClock;
import android.provider.Settings;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import org.json.JSONArray;
import org.json.JSONObject;

/** On-demand process snapshots and explicitly confirmed force-stop of ordinary user apps. */
public final class AppActivity {
    static boolean validPackage(String name) { return name != null && name.matches("[A-Za-z][A-Za-z0-9_]*(\\.[A-Za-z0-9_]+)+"); }
    private static String read(String path) throws Exception {
        File file=new File(path);
        if (file.length()>65536) throw new IllegalStateException("Process data too large");
        byte[] bytes=Files.readAllBytes(file.toPath());
        if(bytes.length>65536)throw new IllegalStateException("Process data too large");
        return new String(bytes,StandardCharsets.UTF_8);
    }
    // PID identity includes start time, avoiding attribution after PID reuse.
    private static long[] sample(int pid) throws Exception {
        String stat=read("/proc/"+pid+"/stat");int end=stat.lastIndexOf(')');
        if(end<0)throw new IllegalArgumentException("Invalid process stat");
        String[] fields=stat.substring(end+2).trim().split("\\s+");
        long ticks=Long.parseLong(fields[11])+Long.parseLong(fields[12]);
        return new long[]{ticks,Long.parseLong(fields[19]),SystemClock.elapsedRealtime()};
    }
    private static long rss(int pid) throws Exception {
        for(String line:read("/proc/"+pid+"/status").split("\\n"))
            if(line.startsWith("VmRSS:"))return Long.parseLong(line.substring(6).trim().split("\\s+")[0]);
        throw new IllegalStateException("Resident memory unavailable");
    }
    static String identityProtection(int uid,int user,int flags,int packages) {
        if(uid/100000!=user || uid%100000<10000)return "System or another Android user";
        if((flags&(ApplicationInfo.FLAG_SYSTEM|ApplicationInfo.FLAG_UPDATED_SYSTEM_APP|ApplicationInfo.FLAG_PERSISTENT))!=0)return "Protected system app";
        if(packages!=1)return "Shared app identity";
        return null;
    }
    private static String protectedReason(Context ctx,PackageManager pm,ActivityManager am,String pkg,int user) throws Exception {
        String stage="package identity";
        try {
        if(!validPackage(pkg))return "Invalid package";
        ApplicationInfo info=pm.getApplicationInfo(pkg,0);
        String[] shared=pm.getPackagesForUid(info.uid);
        String identity=identityProtection(info.uid,user,info.flags,shared==null?0:shared.length);
        if(identity!=null)return identity;
        stage="keyboard preference";
        String ime=Settings.Secure.getString(ctx.getContentResolver(),Settings.Secure.DEFAULT_INPUT_METHOD);
        if(ime!=null && ime.startsWith(pkg+"/"))return "Current keyboard";
        stage="launcher identity";
        Intent home=new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME);
        android.content.pm.ResolveInfo launcher=pm.resolveActivity(home,PackageManager.MATCH_DEFAULT_ONLY);
        if(launcher!=null && launcher.activityInfo!=null && pkg.equals(launcher.activityInfo.packageName))return "Current launcher";
        stage="device-owner policy";
        DevicePolicyManager policy=(DevicePolicyManager)ctx.getSystemService(Context.DEVICE_POLICY_SERVICE);
        if(policy!=null && (policy.isDeviceOwnerApp(pkg)||policy.isProfileOwnerApp(pkg)))return "Device management app";
        stage="active administrator policy";
        if(policy!=null && policy.getActiveAdmins()!=null)for(android.content.ComponentName admin:policy.getActiveAdmins())if(pkg.equals(admin.getPackageName()))return "Device administrator";
        stage="foreground app protection";
        List<ActivityManager.RunningAppProcessInfo> processes=am.getRunningAppProcesses();
        if(processes!=null)for(ActivityManager.RunningAppProcessInfo process:processes)
            if(process.uid==info.uid && process.importance<=ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND)return "Foreground app";
        return null;
        } catch(Exception error) {
            String detail=error.getMessage()==null?"":error.getMessage();
            throw new IllegalStateException(stage+": "+error.getClass().getSimpleName()+": "+detail.substring(0,Math.min(384,detail.length())),error);
        }
    }
    private static JSONObject snapshot(Context ctx,PackageManager pm,ActivityManager am,int user) throws Exception {
        List<ActivityManager.RunningAppProcessInfo> processes=am.getRunningAppProcesses();
        if(processes==null)throw new IllegalStateException("Running app information unavailable");
        Map<Integer,long[]> before=new HashMap<>();
        long start=SystemClock.elapsedRealtime();
        for(ActivityManager.RunningAppProcessInfo process:processes)if(process.uid/100000==user && process.uid%100000>=10000)
            try{before.put(process.pid,sample(process.pid));}catch(Exception ignored){}
        Thread.sleep(1000);
        double seconds=(SystemClock.elapsedRealtime()-start)/1000.0;
        long hz=android.system.Os.sysconf(android.system.OsConstants._SC_CLK_TCK);
        if(hz<=0)throw new IllegalStateException("CPU clock tick rate unavailable");
        Map<String,JSONObject> rows=new HashMap<>();Set<Integer> seen=new HashSet<>();
        for(ActivityManager.RunningAppProcessInfo process:processes){
            if(!seen.add(process.pid)||!before.containsKey(process.pid))continue;
            String[] packages=pm.getPackagesForUid(process.uid);
            if(packages==null||packages.length!=1)continue;String pkg=packages[0];
            if(!validPackage(pkg))continue;
            ApplicationInfo info;try{info=pm.getApplicationInfo(pkg,0);}catch(Exception gone){continue;}
            JSONObject row=rows.get(pkg);
            if(row==null){
                if(rows.size()>=256)continue;
                CharSequence label=info.loadLabel(pm);String name=label==null?pkg:label.toString();
                row=new JSONObject().put("package",pkg).put("label",name.substring(0,Math.min(120,name.length())))
                    .put("rssKb",0).put("cpuPercent",0.0).put("processes",0).put("foreground",false);
                String reason;try{reason=protectedReason(ctx,pm,am,pkg,user);}catch(Exception unavailable){reason="Protection status unavailable";row.put("protectionError",unavailable.getMessage());}
                row.put("canStop",reason==null).put("protectedReason",reason==null?JSONObject.NULL:reason);
                rows.put(pkg,row);
            }
            try{
                long[] old=before.get(process.pid),current=sample(process.pid);
                if(current[1]!=old[1]||current[0]<old[0])continue;
                row.put("cpuPercent",row.getDouble("cpuPercent")+(current[0]-old[0])*100.0/(hz*Math.max(0.001,(current[2]-old[2])/1000.0)));
                row.put("rssKb",row.getLong("rssKb")+rss(process.pid));
                row.put("processes",row.getInt("processes")+1);
                row.put("foreground",row.getBoolean("foreground")||process.importance<=ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND);
            }catch(Exception ignored){ /* Exited or inaccessible process is not a zero-memory reading. */ }
        }
        List<JSONObject> ordered=new ArrayList<>(rows.values());ordered.sort(new java.util.Comparator<JSONObject>() {
            @Override public int compare(JSONObject a,JSONObject b) { return Double.compare(b.optDouble("cpuPercent"),a.optDouble("cpuPercent")); }
        });
        JSONArray apps=new JSONArray();for(JSONObject row:ordered)if(row.getInt("processes")>0)apps.put(row);
        return new JSONObject().put("apps",apps).put("user",user).put("sampleSeconds",seconds).put("sampledAtUptime",SystemClock.elapsedRealtime()/1000)
            .put("note","CPU: 100% is one core. RSS includes shared pages. Cached memory is not proof of battery drain.");
    }
    public static void main(String[] args) throws Exception {
        if(android.os.Process.myUid()!=0)throw new SecurityException("Root required");
        if(args.length<2 || !args[0].matches("[0-9]{1,5}"))throw new IllegalArgumentException("Expected user and operation");
        if(Looper.getMainLooper()==null)Looper.prepareMainLooper();
        Class<?> thread=Class.forName("android.app.ActivityThread");Object activity=thread.getMethod("systemMain").invoke(null);
        Context ctx=(Context)thread.getMethod("getSystemContext").invoke(activity);
        int user=Integer.parseInt(args[0]);
        android.os.UserHandle handle=(android.os.UserHandle)android.os.UserHandle.class.getMethod("of",int.class).invoke(null,user);
        ctx=(Context)Context.class.getMethod("createContextAsUser",android.os.UserHandle.class,int.class).invoke(ctx,handle,0);
        PackageManager pm=ctx.getPackageManager();ActivityManager am=(ActivityManager)ctx.getSystemService(Context.ACTIVITY_SERVICE);
        if(args[1].equals("list") && args.length==2)System.out.println(snapshot(ctx,pm,am,user));
        else if(args[1].equals("stop") && args.length>=4 && args.length<=19 && args[2].equals("--confirm")){
            JSONArray results=new JSONArray();Set<String> seen=new HashSet<>();
            for(int i=3;i<args.length;i++){
                String pkg=args[i];if(!seen.add(pkg))continue;
                JSONObject result=new JSONObject().put("package",pkg);
                try{
                    String reason=protectedReason(ctx,pm,am,pkg,user);
                    if(reason!=null)result.put("result","skipped").put("detail",reason);
                    else{
                        Process child=new ProcessBuilder("/system/bin/am","force-stop","--user",Integer.toString(user),pkg).redirectErrorStream(true).start();
                        if(!child.waitFor(2,TimeUnit.SECONDS)){child.destroyForcibly();result.put("result","failed").put("detail","Android command timed out");}
                        else if(child.exitValue()!=0)result.put("result","failed").put("detail","Android rejected force-stop");
                        else result.put("result","stopped").put("detail","Android accepted force-stop; apps may restart later");
                    }
                }catch(Exception error){result.put("result","failed").put("detail",error.getClass().getSimpleName());}
                results.put(result);
            }
            System.out.println(new JSONObject().put("results",results).put("user",user));
        }else throw new IllegalArgumentException("Invalid app activity operation");
        System.exit(0);
    }
}
