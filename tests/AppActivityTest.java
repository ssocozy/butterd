package dev.butterd.module;
public final class AppActivityTest {
    public static void main(String[] args) {
        if(AppActivity.identityProtection(10218,0,0,1)!=null)throw new AssertionError("Ordinary app incorrectly protected");
        for(int[] sample:new int[][]{{1000,0,0,1},{110218,0,0,1},{10218,0,1,1},{10218,0,128,1},{10218,0,8,1},{10218,0,0,2}})
            if(AppActivity.identityProtection(sample[0],sample[1],sample[2],sample[3])==null)throw new AssertionError("Protected identity was allowed");
        for(String name:new String[]{"com.example.app","com.facebook.orca"})if(!AppActivity.validPackage(name))throw new AssertionError(name);
        for(String name:new String[]{"android","com.app;reboot","com..app","--user"})if(AppActivity.validPackage(name))throw new AssertionError(name);
    }
}
