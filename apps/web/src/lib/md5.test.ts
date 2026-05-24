import { describe, expect, it } from "vitest";
import { handleForUserId, md5 } from "./md5";

describe("md5", () => {
  it("matches RFC 1321 / standard test vectors", () => {
    expect(md5("")).toBe("d41d8cd98f00b204e9800998ecf8427e");
    expect(md5("abc")).toBe("900150983cd24fb0d6963f7d28e17f72");
    expect(md5("The quick brown fox jumps over the lazy dog")).toBe(
      "9e107d9d372bb6826bd81d3542a419d6",
    );
  });

  it("hashes a UUID the same way Postgres md5() does", () => {
    // Postgres: SELECT md5('11111111-1111-1111-1111-111111111111');
    expect(md5("11111111-1111-1111-1111-111111111111")).toBe(
      "38c6cbd28bf165070d070980dd1fb595",
    );
  });

  it("handleForUserId takes the first 8 hex chars of the md5", () => {
    const id = "11111111-1111-1111-1111-111111111111";
    expect(handleForUserId(id)).toBe(md5(id).slice(0, 8));
    expect(handleForUserId(id)).toBe("38c6cbd2");
    expect(handleForUserId(id)).toHaveLength(8);
  });
});
