package rs.glory.counter;

public final class PermissionsTest {
    public static void main(String[] args) {
        // Image, video, selected grants; expected PhotosRead status. Full
        // access must survive an absent optional selected-only grant.
        int[][] cases = {
            {0, 0, 0, 3}, {0, 0, 1, 5},
            {1, 0, 0, 5}, {1, 0, 1, 5},
            {0, 1, 0, 5}, {0, 1, 1, 5},
            {1, 1, 0, 2}, {1, 1, 1, 2}
        };
        for (int[] row : cases) {
            long actual = GloryPermissions.photosStatus(row[0] == 1, row[1] == 1, row[2] == 1);
            if (actual != row[3]) throw new AssertionError("PhotosRead status: " + java.util.Arrays.toString(row) + ", got " + actual);
        }
        System.out.println("Android PhotosRead: eight full/partial/denied combinations passed");
    }
}
