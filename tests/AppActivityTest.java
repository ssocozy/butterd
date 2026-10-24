package dev.butterd.module;
public final class AppActivityTest {
    public static void main(String[] args) {
        if(AppActivity.identityProtection(10218,0,0,1)!=null)throw new AssertionError("Ordinary app incorrectly protected");
        for(int[] sample:new int[][]{{1000,0,0,1},{110218,0,0,1},{10218,0,1,1},{10218,0,128,1},{10218,0,8,1},{10218,0,0,2}})
            if(AppActivity.identityProtection(sample[0],sample[1],sample[2],sample[3])==null)throw new AssertionError("Protected identity was allowed");
        if(!AppActivity.keyboardValue("com.example.keyboard/.Ime\n").equals("com.example.keyboard/.Ime"))throw new AssertionError("Keyboard component rejected");
        if(!AppActivity.keyboardValue("com.example.keyboard/com.example.keyboard.InputService").equals("com.example.keyboard/com.example.keyboard.InputService"))throw new AssertionError("Full keyboard component rejected");
        if(!AppActivity.keyboardValue("null\n").equals("null"))throw new AssertionError("Missing keyboard state rejected");
        for(String response:new String[]{"", "Permission Denial", "com.example.keyboard/", "com.example.keyboard/.Ime\nError"}) {
            boolean rejected=false;try{AppActivity.keyboardValue(response);}catch(IllegalArgumentException expected){rejected=true;}
            if(!rejected)throw new AssertionError("Invalid keyboard output accepted");
        }
        for(String name:new String[]{"com.example.app","com.facebook.orca"})if(!AppActivity.validPackage(name))throw new AssertionError(name);
        for(String name:new String[]{"android","com.app;reboot","com..app","--user"})if(AppActivity.validPackage(name))throw new AssertionError(name);
    }
}
