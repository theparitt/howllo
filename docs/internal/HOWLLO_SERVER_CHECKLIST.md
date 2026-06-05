# Howllo Server Checklist

Checklist นี้ใช้สำหรับไล่งานฝั่ง `howllo-server` แบบ execution-first

หลักการคือ:

- เช็กได้จริง
- ผูกกับ product goal
- แยกตาม phase
- ใช้ได้ทั้งกับ backend implementation และ planning


## 0. Core Rules

- [ ] ทุก record สำคัญมี tenant boundary ชัดเจน
- [ ] ทุก write path มี permission check ใน backend
- [ ] ห้ามให้ frontend เป็นตัว enforce business rule หลัก
- [ ] status ใช้ closed enum เท่านั้น
- [ ] status transition ทุกครั้งเก็บ history
- [ ] AI ไม่อยู่ใน correctness path


## 1. Phase 0: Foundation and Stabilization

### 1.1 Configuration

- [ ] รวม config loading ไว้ในที่เดียว
- [ ] แยก config สำหรับ dev / test / prod
- [ ] ตรวจ env ที่จำเป็นตั้งแต่ startup
- [ ] มี default ที่ปลอดภัยเฉพาะ local dev

### 1.2 Error handling

- [ ] มี app error type กลาง
- [ ] map error เป็น HTTP response แบบสม่ำเสมอ
- [ ] แยก `400`, `401`, `403`, `404`, `409`, `422`, `500` ให้ชัด
- [ ] ลด `InternalServerError` ที่เกิดจาก validation/business rule

### 1.3 Validation

- [ ] validate request body สำหรับ create post
- [ ] validate request body สำหรับ create comment
- [ ] validate request body สำหรับ admin update endpoints
- [ ] validate query params สำหรับ filter/sort/pagination
- [ ] ป้องกัน empty title / empty body / invalid enum values

### 1.4 Status model

- [ ] สร้าง typed status enum ใน backend
- [ ] map ค่า enum กับ database อย่างชัดเจน
- [ ] ห้าม update status ด้วย freeform string
- [ ] normalize public response format ของ status

### 1.5 Logging and observability

- [ ] เพิ่ม structured logging
- [ ] เพิ่ม request ID หรือ correlation ID
- [ ] log auth failures
- [ ] log permission failures
- [ ] log admin mutations
- [ ] log DB/query failures แบบ traceable

### 1.6 Handler cleanup

- [ ] ย้าย business logic ออกจาก HTTP handlers
- [ ] แยก service/use-case layer สำหรับ workflow สำคัญ
- [ ] แยก repository/query logic ออกจาก route handlers เท่าที่จำเป็น

### 1.7 Testing baseline

- [ ] มี integration test สำหรับ health endpoint
- [ ] มี integration test สำหรับ board listing
- [ ] มี integration test สำหรับ post listing
- [ ] มี integration test สำหรับ post detail
- [ ] มี integration test สำหรับ create post
- [ ] มี integration test สำหรับ create comment
- [ ] มี integration test สำหรับ vote add/remove
- [ ] มี integration test สำหรับ admin status update
- [ ] มี permission test สำหรับ member/admin boundary
- [ ] มี test สำหรับ tenant isolation

### 1.8 SQL and CI discipline

- [ ] SQLx query checking ทำงานใน CI
- [ ] migration รันใน test environment ได้
- [ ] ไม่มี query ที่ rely on undocumented behavior


## 2. Phase 1: Core Feedback Loop

### 2.1 Boards

- [ ] มี board detail endpoint
- [ ] มี create board endpoint สำหรับ admin
- [ ] มี edit board endpoint สำหรับ admin
- [ ] มี hide/private policy สำหรับ board
- [ ] private board enforce ผ่าน backend authorization

### 2.2 Posts

- [ ] post listing รองรับ pagination
- [ ] post listing รองรับ status filter
- [ ] post listing รองรับ tag filter
- [ ] post listing รองรับ sort mode
- [ ] มี endpoint สำหรับ author edit post
- [ ] มีกติกาชัดว่า author edit ได้ถึงเมื่อไร
- [ ] มี post hide / soft delete policy
- [ ] ซ่อน hidden post จาก public responses

### 2.3 Tags

- [ ] มี create tag endpoint
- [ ] มี update tag endpoint
- [ ] มี delete tag endpoint
- [ ] มี attach tag to post endpoint
- [ ] มี detach tag from post endpoint
- [ ] enforce tenant-scoped tags

### 2.4 Comments

- [ ] เพิ่ม `comment_type`
- [ ] รองรับอย่างน้อย `user`, `official`, `moderator`
- [ ] official response ใช้ comment model เดียวกัน
- [ ] comment list response มี type ชัดเจน

### 2.5 Follow / subscribe

- [ ] มี model สำหรับ follow/subscription
- [ ] user follow post ได้
- [ ] user unfollow post ได้
- [ ] มี endpoint ดูว่าผู้ใช้ follow post นี้อยู่ไหม
- [ ] เก็บ subscription event สำหรับ status change
- [ ] เก็บ subscription event สำหรับ official response

### 2.6 Status history

- [ ] มี endpoint อ่าน status history ของ post
- [ ] history response มี `who`
- [ ] history response มี `when`
- [ ] history response มี `from`
- [ ] history response มี `to`
- [ ] history response มี `reason` หรือ field ที่เตรียมไว้รองรับ


## 3. Phase 1.5: Duplicate Control

### 3.1 Data model

- [ ] ออกแบบ duplicate relation ระหว่าง post
- [ ] รองรับ canonical post
- [ ] รองรับ duplicate status/flag
- [ ] กำหนด policy ว่า duplicate post ยัง comment/vote ได้หรือไม่

### 3.2 Admin workflow

- [ ] admin mark post as duplicate ได้
- [ ] admin link ไป canonical post ได้
- [ ] admin unmark duplicate ได้
- [ ] duplicate relation audit ได้

### 3.3 Public behavior

- [ ] duplicate post แสดง link ไป post หลักได้
- [ ] API response บอกว่า post นี้เป็น duplicate ของอะไร
- [ ] public clients ไม่ต้องเดา logic เอง


## 4. Phase 2: Moderation and Roadmap

### 4.1 Moderation

- [ ] hide/unhide post endpoint สมบูรณ์
- [ ] lock/unlock post endpoint สมบูรณ์
- [ ] hide/unhide comment endpoint
- [ ] official comment promotion/demotion endpoint
- [ ] moderation note หรือ internal action log

### 4.2 Status workflow

- [ ] define allowed status transitions
- [ ] block invalid transitions
- [ ] status change รองรับ reason
- [ ] status change เก็บ actor ชัดเจน
- [ ] status history ไม่หายแม้ post เปลี่ยนหลายรอบ

### 4.3 Membership and roles

- [ ] owner/admin/member/moderator role model ชัด
- [ ] มี endpoint จัดการ membership
- [ ] admin invite/add member ได้
- [ ] owner/admin เปลี่ยน role ได้
- [ ] member ทำ admin action ไม่ได้

### 4.4 Roadmap resource

- [ ] มี `GET /roadmap`
- [ ] roadmap ไม่ผูกแค่ `GET /posts?status=...`
- [ ] roadmap response ออกแบบให้โตไปเป็น milestone/epic/release ได้
- [ ] roadmap filter ตาม status ได้
- [ ] roadmap filter ตาม tag/group ได้

### 4.5 Public roadmap trust

- [ ] roadmap items แสดง current status
- [ ] roadmap items แสดง status history หรือ latest update summary
- [ ] roadmap items แสดง official updates ได้


## 5. Phase 3: Production, Trust, and Ownership

### 5.1 Export

- [ ] export posts เป็น CSV ได้
- [ ] export posts เป็น JSON ได้
- [ ] export รองรับ tenant scope
- [ ] export ไม่รั่วข้อมูล private/hidden โดยไม่มีสิทธิ์

### 5.2 API and docs

- [ ] มี API documentation
- [ ] มี endpoint contract ชัดเจน
- [ ] มี example request/response สำหรับ core flows

### 5.3 Operations

- [ ] มี health endpoint
- [ ] มี readiness endpoint
- [ ] มี dependency checks สำหรับ DB
- [ ] startup failure อ่านง่าย

### 5.4 Abuse protection

- [ ] rate limit create post
- [ ] rate limit create comment
- [ ] rate limit vote actions
- [ ] ป้องกัน spam หรือ repeated writes ระดับพื้นฐาน

### 5.5 Audit and safety

- [ ] admin mutations ถูก audit
- [ ] audit อ่านย้อนหลังได้
- [ ] sensitive actions trace กลับไปยัง actor ได้
- [ ] มี CORS/trusted-origin policy ชัด

### 5.6 Self-hosting readiness

- [ ] migration ใช้งานง่าย
- [ ] seed dev data หรือ example setup มี
- [ ] docs สำหรับ Postgres + server deployment มี


## 6. Phase 4: Search and Discovery

### 6.1 Search baseline

- [ ] search posts ได้
- [ ] search boards ได้
- [ ] ใช้ PostgreSQL full-text search เป็น baseline

### 6.2 Ranking

- [ ] sort by newest
- [ ] sort by top
- [ ] sort by active
- [ ] sort by most voted

### 6.3 Duplicate prevention

- [ ] suggest similar posts ระหว่าง create post
- [ ] similarity logic usable ได้แม้ไม่มี AI
- [ ] ลด duplicate submissions ได้จริงใน product flow

### 6.4 Discovery endpoints

- [ ] board summary endpoint
- [ ] roadmap summary endpoint
- [ ] aggregation ที่ frontend ใช้ได้โดยไม่ต้องคำนวณเองทั้งหมด


## 7. Phase 5: AI Assist

### 7.1 Guardrails

- [ ] AI อยู่หลัง feature flag
- [ ] AI ใช้งานได้แบบ optional
- [ ] ระบบหลักยังทำงานได้แม้ปิด AI

### 7.2 Duplicate assist

- [ ] AI duplicate suggestion ไม่ auto-merge
- [ ] admin review suggestion ก่อน apply

### 7.3 Moderation assist

- [ ] AI ช่วยจัดกลุ่มหรือ summarize ได้
- [ ] AI ไม่ตัดสิน moderation เอง

### 7.4 Evaluation

- [ ] มีชุดทดสอบคุณภาพ suggestion
- [ ] มี process ตรวจ false positive / false negative


## 8. Future Expansion Hooks

### 8.1 Changelog

- [ ] data model เผื่อเชื่อม `Done` ไป changelog
- [ ] roadmap/done items export ไป release/changelog flow ได้

### 8.2 Release notes

- [ ] done items group เป็น release notes ได้ในอนาคต
- [ ] API shape ไม่ block release entity เพิ่มทีหลัง

### 8.3 Customer portal

- [ ] current model ไม่ปิดทางไปสู่ feedback + roadmap + changelog portal


## 9. Definition of Done for Server V1

- [ ] tenant isolation ครบ
- [ ] membership/role enforcement ครบ
- [ ] create post/comment/vote/follow ครบ
- [ ] tag workflow ครบ
- [ ] duplicate control ใช้งานได้
- [ ] roadmap endpoint แยกชัด
- [ ] status history อ่านได้และเชื่อถือได้
- [ ] official responses อยู่ใน comment timeline
- [ ] export CSV/JSON ใช้งานได้
- [ ] search baseline ใช้งานได้
- [ ] self-hosting docs พร้อม
- [ ] AI ยังเป็น optional layer เท่านั้น


## 10. Suggested Build Order

- [ ] Foundation and stabilization
- [ ] Core feedback loop
- [ ] Follow / subscribe
- [ ] Duplicate control
- [ ] Moderation and roadmap resource
- [ ] Export and production hardening
- [ ] Search and discovery
- [ ] AI assist
