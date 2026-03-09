// Simple Java example
public class Sample {
    private int value;

    public Sample(int value) {
        this.value = value;
    }

    public static int add(int a, int b) {
        return a + b;
    }

    protected void log(String message) {
        System.out.println(message);
    }

    void defaultAccess() {
        // package-private method
    }

    public static void main(String[] args) {
        int result = add(5, 3);
        System.out.println("Result: " + result);
    }
}

interface Service {
    void execute();
}

enum Status { ACTIVE, INACTIVE }
