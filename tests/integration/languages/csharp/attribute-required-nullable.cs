public class UserRecord
{
    [Required]
    public Guid UserId { get; set; }  // ERROR - Required on non-nullable Guid

    [Required]
    public Guid? NullableId { get; set; }  // OK - nullable Guid

    [Required]
    public string Name { get; set; }  // OK - string is reference type
}
