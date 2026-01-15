public static class UserMapper
{
    public static UserDto Map(User user) { return new UserDto(); }  // MATCH - missing 'this'
    public static UserDto MapExt(this User user) { return new UserDto(); }  // OK - is extension
    public static UserDto MapTwo(User a, User b) { return new UserDto(); }  // OK - 2 params
}
