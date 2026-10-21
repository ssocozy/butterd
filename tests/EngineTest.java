package dev.butterd.butter;
import java.util.Properties;
public final class EngineTest {
    private static void expect(boolean value) { if (!value) throw new AssertionError(); }
    public static void main(String[] args) {
        expect(Engine.chooseClock("480000 1200000 2200000", 480000, 1600000) == 1200000);
        expect(Engine.chooseClock("1400000000 800000000 265000000", 265000000, 1000000000) == 800000000);
        try { Engine.chooseClock("2200000 unknown", 0, 2200000); throw new AssertionError(); } catch (IllegalArgumentException expected) { }
        try { Engine.chooseClock("2200000", 0, 1000000); throw new AssertionError(); } catch (IllegalArgumentException expected) { }
        expect(Engine.normalizeTouch("480Hz").equals("1"));
        expect(Engine.normalizeTouch("0").equals("0"));
        try { Engine.normalizeTouch("240Hz 480Hz"); throw new AssertionError(); } catch (IllegalArgumentException expected) { }
        Properties journal = new Properties(); journal.setProperty("expected.0","2200000"); journal.setProperty("before.0","480000");
        expect(Engine.owned(journal,0,"2200000")); expect(!Engine.owned(journal,0,"480000"));
        journal.setProperty("pending.0","true");
        expect(Engine.owned(journal,0,"480000")); expect(!Engine.owned(journal,0,"1000000"));
        System.out.println("Butter clock bounds, touch parsing and interrupted-write ownership checked.");
    }
}
